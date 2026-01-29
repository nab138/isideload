use plist::Dictionary;
use plist_macro::pretty_print_dictionary;
use rootcause::prelude::*;

pub struct SensitivePlistAttachment {
    pub plist: Dictionary,
}

impl SensitivePlistAttachment {
    pub fn new(plist: Dictionary) -> Self {
        SensitivePlistAttachment { plist }
    }

    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // if env variable DEBUG_SENSITIVE is set, print full plist
        if std::env::var("DEBUG_SENSITIVE").is_ok() {
            return writeln!(f, "{}", pretty_print_dictionary(&self.plist));
        }
        writeln!(
            f,
            "<Potentially sensitive data - set DEBUG_SENSITIVE env variable to see contents>"
        )
    }
}

impl std::fmt::Display for SensitivePlistAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

impl std::fmt::Debug for SensitivePlistAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}

pub trait PlistDataExtract {
    fn get_data(&self, key: &str) -> Result<&[u8], Report>;
    fn get_str(&self, key: &str) -> Result<&str, Report>;
    fn get_string(&self, key: &str) -> Result<String, Report>;
    fn get_signed_integer(&self, key: &str) -> Result<i64, Report>;
    fn get_dict(&self, key: &str) -> Result<&Dictionary, Report>;
}

impl PlistDataExtract for Dictionary {
    fn get_data(&self, key: &str) -> Result<&[u8], Report> {
        self.get(key).and_then(|v| v.as_data()).ok_or_else(|| {
            report!("Plist missing data for key '{}'", key)
                .attach(SensitivePlistAttachment::new(self.clone()))
        })
    }

    fn get_str(&self, key: &str) -> Result<&str, Report> {
        self.get(key).and_then(|v| v.as_string()).ok_or_else(|| {
            report!("Plist missing string for key '{}'", key)
                .attach(SensitivePlistAttachment::new(self.clone()))
        })
    }

    fn get_string(&self, key: &str) -> Result<String, Report> {
        self.get(key)
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                report!("Plist missing string for key '{}'", key)
                    .attach(SensitivePlistAttachment::new(self.clone()))
            })
    }

    fn get_signed_integer(&self, key: &str) -> Result<i64, Report> {
        self.get(key)
            .and_then(|v| v.as_signed_integer())
            .ok_or_else(|| {
                report!("Plist missing signed integer for key '{}'", key)
                    .attach(SensitivePlistAttachment::new(self.clone()))
            })
    }

    fn get_dict(&self, key: &str) -> Result<&Dictionary, Report> {
        self.get(key)
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| {
                report!("Plist missing dictionary for key '{}'", key)
                    .attach(SensitivePlistAttachment::new(self.clone()))
            })
    }
}
