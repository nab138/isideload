use std::{collections::HashMap, sync::Mutex};

use base64::prelude::*;
use rootcause::prelude::*;

pub trait SideloadingStorage: Send + Sync {
    fn store(&self, key: &str, value: &str) -> Result<(), Report>;
    fn retrieve(&self, key: &str) -> Result<Option<String>, Report>;

    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), Report> {
        self.store(key, &BASE64_STANDARD.encode(value))
    }

    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, Report> {
        if let Some(value) = self.retrieve(key)? {
            Ok(Some(BASE64_STANDARD.decode(value)?))
        } else {
            Ok(None)
        }
    }

    fn delete(&self, key: &str) -> Result<(), Report> {
        self.store(key, "")
    }
}

pub fn new_storage() -> impl SideloadingStorage {
    #[cfg(feature = "keyring-storage")]
    {
        crate::util::keyring_storage::KeyringStorage::default()
    }
    #[cfg(not(feature = "keyring-storage"))]
    {
        InMemoryStorage::new()
    }
}

pub struct InMemoryStorage {
    storage: Mutex<HashMap<String, String>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        InMemoryStorage {
            storage: Mutex::new(HashMap::new()),
        }
    }
}

impl SideloadingStorage for InMemoryStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), Report> {
        let mut storage = self.storage.lock().unwrap();
        storage.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, Report> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<(), Report> {
        let mut storage = self.storage.lock().unwrap();
        storage.remove(key);
        Ok(())
    }
}
