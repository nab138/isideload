use plist_macro::{plist_to_xml_bytes, plist_value_to_xml_bytes, pretty_print_dictionary};
use rootcause::prelude::*;

pub fn plist_to_xml_string(p: &plist::Dictionary) -> String {
    String::from_utf8(plist_to_xml_bytes(p)).unwrap()
}

pub fn plist_value_to_xml_string(p: &plist::Value) -> String {
    String::from_utf8(plist_value_to_xml_bytes(p)).unwrap()
}

pub trait PlistDataExtract {
    fn get_data(&self, key: &str) -> Result<&[u8], Report>;
    fn get_str(&self, key: &str) -> Result<&str, Report>;
    fn get_string(&self, key: &str) -> Result<String, Report>;
    fn get_signed_integer(&self, key: &str) -> Result<i64, Report>;
    fn get_dict(&self, key: &str) -> Result<&plist::Dictionary, Report>;
}

impl PlistDataExtract for plist::Dictionary {
    fn get_data(&self, key: &str) -> Result<&[u8], Report> {
        self.get(key).and_then(|v| v.as_data()).ok_or_else(|| {
            report!("Plist missing data for key '{}'", key).attach(pretty_print_dictionary(self))
        })
    }

    fn get_str(&self, key: &str) -> Result<&str, Report> {
        self.get(key).and_then(|v| v.as_string()).ok_or_else(|| {
            report!("Plist missing string for key '{}'", key).attach(pretty_print_dictionary(self))
        })
    }

    fn get_string(&self, key: &str) -> Result<String, Report> {
        self.get(key)
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                report!("Plist missing string for key '{}'", key)
                    .attach(pretty_print_dictionary(self))
            })
    }

    fn get_signed_integer(&self, key: &str) -> Result<i64, Report> {
        self.get(key)
            .and_then(|v| v.as_signed_integer())
            .ok_or_else(|| {
                report!("Plist missing signed integer for key '{}'", key)
                    .attach(pretty_print_dictionary(self))
            })
    }

    fn get_dict(&self, key: &str) -> Result<&plist::Dictionary, Report> {
        self.get(key)
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| {
                report!("Plist missing dictionary for key '{}'", key)
                    .attach(pretty_print_dictionary(self))
            })
    }
}
