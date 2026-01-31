use crate::{
    anisette::{AnisetteProvider, AnisetteProviderConfig, remote_v3::state::AnisetteState},
    auth::grandslam::GrandSlam,
};
use plist::Dictionary;
use plist_macro::plist;
use reqwest::header::HeaderMap;
use rootcause::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, time::SystemTime};

#[derive(Deserialize, Debug, Clone)]
pub struct AnisetteClientInfo {
    pub client_info: String,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub struct AnisetteData {
    pub routing_info: String,
    pub machine_id: String,
    pub one_time_password: String,
    pub device_description: String,
    pub device_unique_identifier: String,
    pub local_user_id: String,
    pub generated_at: SystemTime,
}

// Some headers don't seem to be required. I guess not including them is technically more efficient soooo
impl AnisetteData {
    pub fn get_headers(&self) -> HashMap<String, String> {
        //let dt: DateTime<Utc> = Utc::now().round_subsecs(0);

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

    pub fn get_header_map(&self) -> HeaderMap {
        let headers_map = self.get_headers();
        let mut header_map = HeaderMap::new();

        for (key, value) in headers_map {
            header_map.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                reqwest::header::HeaderValue::from_str(&value).unwrap(),
            );
        }

        header_map
    }

    pub fn get_client_provided_data(&self) -> Dictionary {
        let headers = self.get_headers();

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

    pub fn needs_refresh(&self) -> bool {
        let elapsed = self.generated_at.elapsed().unwrap();
        elapsed.as_secs() > 60
    }
}

pub struct RenewableAnisetteData {
    config: AnisetteProviderConfig,
    anisette_data: Option<AnisetteData>,
    client_info: Option<AnisetteClientInfo>,
    state: Option<AnisetteState>,
}

impl RenewableAnisetteData {
    pub fn new(config: AnisetteProviderConfig) -> Self {
        RenewableAnisetteData {
            config,
            anisette_data: None,
            client_info: None,
            state: None,
        }
    }

    pub async fn get_anisette_data(&mut self, gs: &mut GrandSlam) -> Result<&AnisetteData, Report> {
        if self
            .anisette_data
            .as_ref()
            .map_or(true, |data| data.needs_refresh())
        {
            if self.client_info.is_none() || self.state.is_none() {
                let mut provider = self.config.get_provider(self.client_info.clone());
                let client_info = provider.get_client_info().await?;
                self.client_info = Some(client_info);
                let data = provider.get_anisette_data(gs).await?;
                self.anisette_data = Some(data);
            } else {
            }
        }

        Ok(self.anisette_data.as_ref().unwrap())
    }

    pub async fn get_client_info(
        &mut self,
        gs: &mut GrandSlam,
    ) -> Result<AnisetteClientInfo, Report> {
        self.get_anisette_data(gs).await?;

        if let Some(client_info) = &self.client_info {
            return Ok(client_info.clone());
        } else {
            bail!("Anisette client info not available");
        }
    }
}
