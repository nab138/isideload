pub mod remote_v3;

use crate::auth::grandslam::GrandSlam;
use chrono::{DateTime, SubsecRound, Utc};
use plist::Dictionary;
use plist_macro::plist;
use reqwest::header::HeaderMap;
use rootcause::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct AnisetteClientInfo {
    pub client_info: String,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub struct AnisetteData {
    machine_id: String,
    one_time_password: String,
    pub routing_info: String,
    device_description: String,
    device_unique_identifier: String,
    local_user_id: String,
}

impl AnisetteData {
    pub fn get_headers(&self, serial: String) -> HashMap<String, String> {
        // let dt: DateTime<Utc> = Utc::now().round_subsecs(0);

        HashMap::from_iter(
            [
                // (
                //     "X-Apple-I-Client-Time".to_string(),
                //     dt.format("%+").to_string().replace("+00:00", "Z"),
                // ),
                // ("X-Apple-I-SRL-NO".to_string(), serial),
                // ("X-Apple-I-TimeZone".to_string(), "UTC".to_string()),
                // ("X-Apple-Locale".to_string(), "en_US".to_string()),
                // ("X-Apple-I-MD-RINFO".to_string(), self.routing_info.clone()),
                // ("X-Apple-I-MD-LU".to_string(), self.local_user_id.clone()),
                (
                    "X-Mme-Device-Id".to_string(),
                    self.device_unique_identifier.clone(),
                ),
                ("X-Apple-I-MD".to_string(), self.one_time_password.clone()),
                ("X-Apple-I-MD-M".to_string(), self.machine_id.clone()),
                // (
                //     "X-Mme-Client-Info".to_string(),
                //     self.device_description.clone(),
                // ),
            ]
            .into_iter(),
        )
    }

    pub fn get_header_map(&self, serial: String) -> HeaderMap {
        let headers_map = self.get_headers(serial);
        let mut header_map = HeaderMap::new();

        for (key, value) in headers_map {
            header_map.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                reqwest::header::HeaderValue::from_str(&value).unwrap(),
            );
        }

        header_map
    }

    pub fn get_client_provided_data(&self, serial: String) -> Dictionary {
        let headers = self.get_headers(serial);

        let mut cpd = plist!(dict {
            "bootstrap": "true",
            "icscrec": "true",
            "loc": "en_US",
            "pbe": "false",
            "prkgen": "true",
            "svct": "iCloud"
        });

        for (key, value) in headers {
            cpd.insert(key.to_string(), plist::Value::String(value));
        }

        cpd
    }
}

#[async_trait::async_trait]
pub trait AnisetteProvider {
    async fn get_anisette_data(&mut self, gs: &mut GrandSlam) -> Result<AnisetteData, Report>;

    async fn get_client_info(&mut self) -> Result<AnisetteClientInfo, Report>;
}
