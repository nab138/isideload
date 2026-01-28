use plist::Dictionary;
use plist_macro::plist;
use rootcause::prelude::*;
use tracing::debug;
use uuid::Uuid;

use crate::{
    anisette::AnisetteData,
    auth::{
        apple_account::{AppToken, AppleAccount},
        grandslam::GrandSlam,
    },
    dev::{
        device_type::DeveloperDeviceType,
        structures::{DeveloperTeam, ListTeamResponse},
    },
    util::plist::{PlistDataExtract, plist_to_xml_string},
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

    pub async fn send_developer_request(
        &self,
        url: &str,
        body: Option<Dictionary>,
    ) -> Result<String, Report> {
        let body = body.unwrap_or_else(|| Dictionary::new());

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

        // let dict: Dictionary = plist::from_bytes(text.as_bytes())
        //     .context("Failed to parse developer request plist")?;

        Ok(text)
    }

    pub async fn list_teams(&self) -> Result<Vec<DeveloperTeam>, Report> {
        let res = self
            .send_developer_request(&DeveloperDeviceType::Any.dev_url("listTeams"), None)
            .await?;

        let response: ListTeamResponse = plist::from_bytes(res.as_bytes())
            .context("Failed to parse list teams response plist")?;

        debug!("List Teams Response Code: {:?}", response.result_code);

        Ok(response.teams)
    }
}
