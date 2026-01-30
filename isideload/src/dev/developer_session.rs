use plist::{Dictionary, Value};
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
    dev::structures::{
        DeveloperDeviceType::{self, *},
        *,
    },
    util::plist::{PlistDataExtract, SensitivePlistAttachment},
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

    pub async fn list_teams(&self) -> Result<Vec<DeveloperTeam>, Report> {
        let response: Vec<DeveloperTeam> = self
            .send_dev_request(&dev_url("listTeams", Any), None, "teams")
            .await
            .context("Failed to list developer teams")?;

        Ok(response)
    }

    pub async fn list_devices(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<Vec<DeveloperDevice>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let devices: Vec<DeveloperDevice> = self
            .send_dev_request(&dev_url("listDevices", device_type), body, "devices")
            .await
            .context("Failed to list developer devices")?;

        Ok(devices)
    }

    pub async fn add_device(
        &self,
        team: &DeveloperTeam,
        name: &str,
        udid: &str,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<DeveloperDevice, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "name": name,
            "deviceNumber": udid,
        });

        let device: DeveloperDevice = self
            .send_dev_request(&dev_url("addDevice", device_type), body, "device")
            .await
            .context("Failed to add developer device")?;

        Ok(device)
    }

    pub async fn list_all_development_certs(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<Vec<DevelopmentCertificate>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let certs: Vec<DevelopmentCertificate> = self
            .send_dev_request(
                &dev_url("listAllDevelopmentCerts", device_type),
                body,
                "certificates",
            )
            .await
            .context("Failed to list development certificates")?;

        Ok(certs)
    }

    pub async fn revoke_development_cert(
        &self,
        team: &DeveloperTeam,
        serial_number: &str,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<(), Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "serialNumber": serial_number,
        });

        self.send_dev_request_no_response(
            &dev_url("revokeDevelopmentCert", device_type),
            Some(body),
        )
        .await
        .context("Failed to revoke development certificate")?;

        Ok(())
    }

    pub async fn submit_development_csr(
        &self,
        team: &DeveloperTeam,
        csr_content: String,
        machine_name: String,
        device_type: impl Into<Option<DeveloperDeviceType>>,
    ) -> Result<CertRequest, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "csrContent": csr_content,
            "machineName": machine_name,
            "machineId": Uuid::new_v4().to_string().to_uppercase(),
        });

        let cert: CertRequest = self
            .send_dev_request(
                &dev_url("submitDevelopmentCSR", device_type),
                body,
                "certRequest",
            )
            .await
            .context("Failed to submit development CSR")?;

        Ok(cert)
    }

    pub async fn list_app_ids(&self, team: &DeveloperTeam) -> Result<ListAppIdsResponse, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let response: Value = self
            .send_dev_request_no_response(&dev_url("listAppIds", Any), body)
            .await
            .context("Failed to list developer app IDs")?
            .into();

        let app_ids: ListAppIdsResponse = plist::from_value(&response).map_err(|e| {
            report!("Failed to deserialize app id response: {:?}", e).attach(
                SensitivePlistAttachment::new(response.as_dictionary().clone().unwrap_or_default()),
            )
        })?;

        Ok(app_ids)
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
