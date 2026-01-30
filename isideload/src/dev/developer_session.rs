use plist::Dictionary;
use plist_macro::{plist, plist_to_xml_string};
use rootcause::prelude::*;
use serde::de::DeserializeOwned;
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    anisette::AnisetteData,
    auth::{
        apple_account::{AppToken, AppleAccount},
        grandslam::GrandSlam,
    },
    util::plist::PlistDataExtract,
};

pub use super::app_ids::*;
pub use super::certificates::*;
pub use super::device_type::DeveloperDeviceType;
pub use super::devices::*;
pub use super::teams::*;

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

    async fn send_dev_request_internal(
        &self,
        url: &str,
        body: impl Into<Option<Dictionary>>,
    ) -> Result<(Dictionary, Option<String>), Report> {
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

        let dict: Dictionary = plist::from_bytes(text.as_bytes())
            .context("Failed to parse developer request plist")?;

        // All this error handling is here to ensure that:
        // 1. We always warn/log errors from the server even if it returns the expected data
        // 2. We return server errors if the expected data is missing
        // 3. We return parsing errors if there is no server error but the expected data is missing
        let response_code = dict.get("resultCode").and_then(|v| v.as_signed_integer());
        let mut server_error: Option<String> = None;
        if let Some(code) = response_code {
            if code != 0 {
                let user_string = dict
                    .get("userString")
                    .and_then(|v| v.as_string())
                    .unwrap_or("Developer request failed.");

                let result_string = dict
                    .get("resultString")
                    .and_then(|v| v.as_string())
                    .unwrap_or("No error message given.");

                // if user and result string match, only show one
                if user_string == result_string {
                    server_error = Some(format!("{} Code: {}", user_string, code));
                } else {
                    server_error =
                        Some(format!("{} Code: {}; {}", user_string, code, result_string));
                }
                error!(server_error);
            }
        } else {
            warn!("No resultCode in developer request response");
        }

        Ok((dict, server_error))
    }

    pub async fn send_dev_request<T: DeserializeOwned>(
        &self,
        url: &str,
        body: impl Into<Option<Dictionary>>,
        response_key: &str,
    ) -> Result<T, Report> {
        let (dict, server_error) = self.send_dev_request_internal(url, body).await?;

        let result: Result<T, _> = dict.get_struct(response_key);

        if let Err(_) = &result {
            if let Some(err) = server_error {
                bail!(err);
            }
        }

        Ok(result.context("Failed to extract developer request result")?)
    }

    pub async fn send_dev_request_no_response(
        &self,
        url: &str,
        body: impl Into<Option<Dictionary>>,
    ) -> Result<Dictionary, Report> {
        let (dict, server_error) = self.send_dev_request_internal(url, body).await?;

        if let Some(err) = server_error {
            bail!(err);
        }

        Ok(dict)
    }
}
