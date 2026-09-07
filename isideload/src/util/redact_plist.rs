use plist_macro::plist_value_to_xml_string;
use rootcause::prelude::*;
use sha1::{Digest, digest::Update};

use crate::util::storage::{SideloadingStorage, new_storage};

pub fn redact_plist(plist: &mut plist::Dictionary, keys: &[&str]) -> Result<(), Report> {
    for key in keys {
        if !redact_plist_key(plist, key)? {
            if let Some(plist::Value::Dictionary(response)) = plist.get_mut("Response") {
                redact_plist_key(response, key)?;
            }
            if let Some(plist::Value::Dictionary(response)) = plist.get_mut("Request") {
                redact_plist_key(response, key)?;
            }
        }
    }

    Ok(())
}

fn redact_plist_key(plist: &mut plist::Dictionary, key: &str) -> Result<bool, Report> {
    let key_parts: Vec<&str> = key.split('/').collect();
    redact_plist_value(plist, &key_parts)
}

fn redact_plist_value(value: &mut plist::Dictionary, key_parts: &[&str]) -> Result<bool, Report> {
    let Some((part, remaining_parts)) = key_parts.split_first() else {
        return Ok(false);
    };

    let Some(value) = value.get_mut(*part) else {
        return Ok(false);
    };

    if remaining_parts.is_empty() {
        let storage = new_storage();
        let salt = match storage.retrieve_data("redaction-salt")? {
            Some(s) => s,
            None => {
                let salt = rand::random::<[u8; 16]>();
                storage.store_data("redaction-salt", &salt)?;
                salt.to_vec()
            }
        };
        println!("lets get salty!, {:?}", salt);
        let hash = sha1::Sha1::new()
            .chain(plist_value_to_xml_string(value))
            .chain(salt)
            .finalize();
        let hash = hash
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();
        *value = plist::Value::String(format!("[redacted] (hash: {})", hash));
        return Ok(true);
    }

    match value {
        plist::Value::Dictionary(nested_dict) => redact_plist_value(nested_dict, remaining_parts),
        plist::Value::Array(values) => values
            .iter_mut()
            .filter_map(|value| match value {
                plist::Value::Dictionary(nested_dict) => Some(nested_dict),
                _ => None,
            })
            .map(|nested_dict| redact_plist_value(nested_dict, remaining_parts))
            .fold(Ok(false), |redacted, nested_redacted| {
                Ok(redacted? || nested_redacted?)
            }),
        _ => Ok(false),
    }
}
