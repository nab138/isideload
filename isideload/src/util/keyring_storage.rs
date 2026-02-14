use crate::util::storage::SideloadingStorage;
use keyring::Entry;
use rootcause::prelude::*;

pub struct KeyringStorage {
    pub service_name: String,
}

impl KeyringStorage {
    pub fn new(service_name: String) -> Self {
        KeyringStorage { service_name }
    }
}

impl Default for KeyringStorage {
    fn default() -> Self {
        KeyringStorage {
            service_name: "isideload".to_string(),
        }
    }
}

impl SideloadingStorage for KeyringStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), Report> {
        Entry::new(&self.service_name, key)?.set_password(value)?;
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, Report> {
        let entry = Entry::new(&self.service_name, key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, key: &str) -> Result<(), Report> {
        let entry = Entry::new(&self.service_name, key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
