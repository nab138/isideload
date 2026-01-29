use plist::Dictionary;
use plist_macro::{plist, plist_to_xml_string};
use rootcause::prelude::*;
use serde::de::DeserializeOwned;
use tracing::warn;
use uuid::Uuid;

use crate::{
    anisette::AnisetteData,
    auth::{
        apple_account::{AppToken, AppleAccount},
        grandslam::GrandSlam,
    },
    dev::structures::{
        DeveloperDevice,
        DeveloperDeviceType::{self, *},
        DeveloperTeam, ListDevicesResponse, ListTeamsResponse,
    },
    util::plist::PlistDataExtract,
};

pub struct DeveloperSession<'a> {
    token: AppToken,
    adsid: String,
    client: &'a GrandSlam,
    anisette_data: &'a AnisetteData,
}

impl<'a> DeveloperSession<'a> {
    pub fn new(
        token: AppToken,
        adsid: String,
        client: &'a GrandSlam,
        anisette_data: &'a AnisetteData,
    ) -> Self {
        DeveloperSession {
            token,
            adsid,
            client,
            anisette_data,
        }
    }

    pub async fn from_account(account: &'a mut AppleAccount) -> Result<Self, Report> {
        let token = account
            .get_app_token("xcode.auth")
            .await
            .context("Failed to get xcode token from Apple account")?;

        let spd = account
            .spd
            .as_ref()
            .ok_or_else(|| report!("SPD not available, cannot get adsid"))?;

        Ok(DeveloperSession::new(
            token,
            spd.get_string("adsid")?,
            &account.grandslam_client,
            &account.anisette_data,
        ))
    }

    pub async fn send_developer_request<T: DeserializeOwned>(
        &self,
        url: &str,
        body: impl Into<Option<Dictionary>>,
    ) -> Result<T, Report> {
        let body = body.into().unwrap_or_else(|| Dictionary::new());

        let base = plist!(dict {
            "clientId": "XABBG36SBA",
            "protocolVersion": "QH65B2",
            "requestId": Uuid::new_v4().to_string().to_uppercase(),
            "userLocale": ["en_US"],
        });

        let body = base.into_iter().chain(body.into_iter()).collect();

        let text = self
            .client
            .post(url)?
            .body(plist_to_xml_string(&body))
            .header("X-Apple-GS-Token", &self.token.token)
            .header("X-Apple-I-Identity-Id", &self.adsid)
            .headers(self.anisette_data.get_header_map())
            .send()
            .await?
            .error_for_status()
            .context("Developer request failed")?
            .text()
            .await
            .context("Failed to read developer request response text")?;

        let dict: T = plist::from_bytes(text.as_bytes())
            .context("Failed to parse developer request plist")?;

        Ok(dict)
    }

    pub async fn list_teams(&self) -> Result<Vec<DeveloperTeam>, Report> {
        let response: ListTeamsResponse = self
            .send_developer_request(&dev_url("listTeams", Any), None)
            .await
            .context("Failed to list developer teams")?;

        if response.result_code != 0 {
            warn!(
                "Non-zero list teams response code: {}",
                response.result_code
            )
        };

        Ok(response.teams)
    }

    pub async fn list_devices(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<Vec<DeveloperDevice>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let response: ListDevicesResponse = self
            .send_developer_request(&dev_url("listDevices", device_type), body)
            .await
            .context("Failed to list developer devices")?;

        if response.result_code != 0 {
            warn!(
                "Non-zero list devices response code: {}",
                response.result_code
            )
        };

        Ok(response.devices)
    }
}

fn dev_url(endpoint: &str, device_type: impl Into<Option<DeveloperDeviceType>>) -> String {
    format!(
        "https://developerservices2.apple.com/services/QH65B2/{}{}.action?clientId=XABBG36SBA",
        device_type
            .into()
            .unwrap_or(DeveloperDeviceType::Ios)
            .url_segment(),
        endpoint,
    )
}
