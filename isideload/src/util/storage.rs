use base64::prelude::*;
use rootcause::prelude::*;

pub trait SideloadingStorage: Send + Sync {
    fn store(&self, key: &str, value: &str) -> Result<(), Report>;
    fn retrieve(&self, key: &str) -> Result<Option<String>, Report>;

    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), Report> {
        let encoded = BASE64_STANDARD.encode(value);
        self.store(key, &encoded)
    }

    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, Report> {
        if let Some(encoded) = self.retrieve(key)? {
            let decoded = BASE64_STANDARD.decode(&encoded)?;
            Ok(Some(decoded))
        } else {
            Ok(None)
        }
    }
}
