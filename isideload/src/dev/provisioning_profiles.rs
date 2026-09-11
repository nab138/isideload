use crate::{
    dev::{
        app_ids::AppId,
        developer_session::DeveloperSession,
        device_type::{DeveloperDeviceType, dev_url},
        devices::DeveloperDevice,
        teams::DeveloperTeam,
    },
    sideload::cert_identity::CertificateIdentity,
};
use plist::{Data, Date};
use plist_macro::plist;
use rootcause::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub encoded_profile: Data,
    pub filename: String,
    pub provisioning_profile_id: String,
    pub name: String,
    pub status: String,
    pub r#type: String,
    pub distribution_method: String,
    pub pro_pro_platorm: Option<String>,
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub date_expire: Date,
    pub managing_app: Option<String>,
    pub app_id_id: String,
    pub is_template_profile: bool,
    pub is_team_profile: Option<bool>,
    pub is_free_provisioning_profile: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedProvisioningProfile {
    pub provisioning_profile_id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub r#type: Option<String>,
    pub platform: Option<String>,
    pub date_expire: Date,
    pub app_id: Option<AppId>,
    pub device_ids: Option<Vec<String>>,
    pub is_free_provisioning_profile: Option<bool>,
    pub is_team_profile: Option<bool>,
    pub profile_type: Option<String>,
}

#[cfg_attr(feature = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(feature = "wasm"), async_trait::async_trait)]
pub trait ProfilesApi {
    fn developer_session(&mut self) -> &mut DeveloperSession;

    async fn list_team_provisioning_profiles(
        &mut self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Vec<ListedProvisioningProfile>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "includeTeamProfiles": true,
        });

        let response: Vec<ListedProvisioningProfile> = self
            .developer_session()
            .send_dev_request(
                &dev_url("listProvisioningProfiles", device_type),
                body,
                "provisioningProfiles",
            )
            .await
            .context("Failed to list provisioning profiles")?;

        Ok(response)
    }

    async fn download_team_provisioning_profile(
        &mut self,
        team: &DeveloperTeam,
        app_id: &AppId,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Profile, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "appIdId": &app_id.app_id_id,
        });

        let response: Profile = self
            .developer_session()
            .send_dev_request(
                &dev_url("downloadTeamProvisioningProfile", device_type),
                body,
                "provisioningProfile",
            )
            .await
            .context("Failed to download provisioning profile")?;

        Ok(response)
    }

    async fn download_provisioning_profile(
        &mut self,
        team: &DeveloperTeam,
        profile_id: &str,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Profile, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "provisioningProfileId": profile_id,
        });

        let response: Profile = self
            .developer_session()
            .send_dev_request(
                &dev_url("downloadProvisioningProfile", device_type),
                body,
                "provisioningProfile",
            )
            .await
            .context("Failed to download provisioning profile")?;

        Ok(response)
    }

    async fn create_provisioning_profile(
        &mut self,
        team: &DeveloperTeam,
        certificiate: &CertificateIdentity,
        app_id: &AppId,
        device: &DeveloperDevice,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Profile, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "provisioningProfileName": format!("{}.{}", &app_id.name, &team.team_id),
            "appIdId": &app_id.app_id_id,
            "distributionType": "limited",
            "certificateIds": [&certificiate.cert_id],
            "deviceIds": [&device.device_id]
        });

        let response: Profile = self
            .developer_session()
            .send_dev_request(
                &dev_url("createProvisioningProfile", device_type),
                body,
                "provisioningProfile",
            )
            .await
            .context("Failed to download provisioning profile")?;

        Ok(response)
    }
}

impl ProfilesApi for DeveloperSession {
    fn developer_session(&mut self) -> &mut DeveloperSession {
        self
    }
}
