use std::path::PathBuf;

use chrono::{DateTime, SubsecRound, Utc};
use reqwest::header::{HeaderMap, HeaderValue};

use crate::SideloadResult as Result;
use crate::anisette::AnisetteProvider;

pub const DEFAULT_ANISETTE_V3_URL: &str = "https://ani.sidestore.io";

pub struct RemoteV3AnisetteProvider {
    url: String,
    config_path: PathBuf,
    serial_number: String,
}

impl RemoteV3AnisetteProvider {
    /// Create a new RemoteV3AnisetteProvider with the given URL and config path
    ///
    /// # Arguments
    /// - `url`: The URL of the remote anisette service
    /// - `config_path`: The path to the config file
    /// - `serial_number`: The serial number of the device
    pub fn new(url: &str, config_path: PathBuf, serial_number: String) -> Self {
        Self {
            url: url.to_string(),
            config_path,
            serial_number,
        }
    }

    pub fn set_url(mut self, url: &str) -> RemoteV3AnisetteProvider {
        self.url = url.to_string();
        self
    }

    pub fn set_config_path(mut self, config_path: PathBuf) -> RemoteV3AnisetteProvider {
        self.config_path = config_path;
        self
    }

    pub fn set_serial_number(mut self, serial_number: String) -> RemoteV3AnisetteProvider {
        self.serial_number = serial_number;
        self
    }
}

impl Default for RemoteV3AnisetteProvider {
    fn default() -> Self {
        Self::new(DEFAULT_ANISETTE_V3_URL, PathBuf::new(), "0".to_string())
    }
}

#[derive(Debug)]
pub struct AnisetteData {
    machine_id: String,
    one_time_password: String,
    routing_info: String,
    device_description: String,
    device_unique_identifier: String,
    local_user_id: String,
}

impl AnisetteData {
    pub fn get_headers(&self, serial: String) -> Result<HeaderMap> {
        let dt: DateTime<Utc> = Utc::now().round_subsecs(0);

        let mut headers = HeaderMap::new();

        for (key, value) in vec![
            (
                "X-Apple-I-Client-Time",
                dt.format("%+").to_string().replace("+00:00", "Z"),
            ),
            ("X-Apple-I-SRL-NO", serial),
            ("X-Apple-I-TimeZone", "UTC".to_string()),
            ("X-Apple-Locale", "en_US".to_string()),
            ("X-Apple-I-MD-RINFO", self.routing_info.clone()),
            ("X-Apple-I-MD-LU", self.local_user_id.clone()),
            ("X-Mme-Device-Id", self.device_unique_identifier.clone()),
            ("X-Apple-I-MD", self.one_time_password.clone()),
            ("X-Apple-I-MD-M", self.machine_id.clone()),
            ("X-Mme-Client-Info", self.device_description.clone()),
        ] {
            headers.insert(key, HeaderValue::from_str(&value)?);
        }

        Ok(headers)
    }
}

impl AnisetteProvider for RemoteV3AnisetteProvider {}
