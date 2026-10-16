use idevice::{IdeviceService, lockdown::LockdownClient, provider::IdeviceProvider};
use rootcause::prelude::*;

pub struct IdeviceInfo {
    pub name: String,
    pub udid: String,
    pub product_type: Option<String>,
}

impl IdeviceInfo {
    pub fn new(name: String, udid: String) -> Self {
        Self {
            name,
            udid,
            product_type: None,
        }
    }

    pub async fn from_device(device: &impl IdeviceProvider) -> Result<Self, Report> {
        let mut lockdown = LockdownClient::connect(device)
            .await
            .context("Failed to connect to device lockdown")?;
        let pairing = device
            .get_pairing_file()
            .await
            .context("Failed to get device pairing file")?;
        lockdown
            .start_session(&pairing)
            .await
            .context("Failed to start lockdown session")?;
        let device_name = lockdown
            .get_value(Some("DeviceName"), None)
            .await
            .context("Failed to get device name")?
            .as_string()
            .ok_or_else(|| report!("Device name is not a string"))?
            .to_string();

        let device_udid = lockdown
            .get_value(Some("UniqueDeviceID"), None)
            .await
            .context("Failed to get device UDID")?
            .as_string()
            .ok_or_else(|| report!("Device UDID is not a string"))?
            .to_string();

        let product_type = lockdown
            .get_value(Some("ProductType"), None)
            .await
            .ok()
            .and_then(|value| value.as_string().map(str::to_string));

        Ok(Self {
            name: device_name,
            udid: device_udid,
            product_type,
        })
    }
}
