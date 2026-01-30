use crate::dev::{
    developer_session::DeveloperSession,
    device_type::{DeveloperDeviceType, dev_url},
    teams::DeveloperTeam,
};
use plist_macro::plist;
use rootcause::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperDevice {
    pub name: Option<String>,
    pub device_id: Option<String>,
    pub device_number: String,
    pub status: Option<String>,
}

#[async_trait::async_trait]
pub trait DevicesApi {
    fn developer_session(&self) -> &DeveloperSession<'_>;

    async fn list_devices(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Vec<DeveloperDevice>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let devices: Vec<DeveloperDevice> = self
            .developer_session()
            .send_dev_request(&dev_url("listDevices", device_type), body, "devices")
            .await
            .context("Failed to list developer devices")?;

        Ok(devices)
    }

    async fn add_device(
        &self,
        team: &DeveloperTeam,
        name: &str,
        udid: &str,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<DeveloperDevice, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "name": name,
            "deviceNumber": udid,
        });

        let device: DeveloperDevice = self
            .developer_session()
            .send_dev_request(&dev_url("addDevice", device_type), body, "device")
            .await
            .context("Failed to add developer device")?;

        Ok(device)
    }
}

impl DevicesApi for DeveloperSession<'_> {
    fn developer_session(&self) -> &DeveloperSession<'_> {
        self
    }
}
