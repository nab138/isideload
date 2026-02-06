use crate::util::storage::SideloadingStorage;
use keyring::Entry;
use rootcause::prelude::*;

pub struct KeyringStorage {}

impl KeyringStorage {
    pub fn new() -> Self {
        KeyringStorage {}
    }
}

impl SideloadingStorage for KeyringStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), Report> {
        Entry::new("isideload", key)?.set_password(value)?;
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, Report> {
        let entry = Entry::new("isideload", key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, key: &str) -> Result<(), Report> {
        let entry = Entry::new("isideload", key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
