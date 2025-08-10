use idevice::{
    IdeviceService,
    afc::AfcClient,
    installation_proxy::InstallationProxyClient,
    lockdown::LockdownClient,
    usbmuxd::{UsbmuxdAddr, UsbmuxdConnection},
};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::{future::Future, path::Path};

use crate::Error;

#[derive(Deserialize, Serialize, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub id: u32,
    pub uuid: String,
}

pub async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let usbmuxd = UsbmuxdConnection::default().await;
    if usbmuxd.is_err() {
        eprintln!("Failed to connect to usbmuxd: {:?}", usbmuxd.err());
        return Err("Failed to connect to usbmuxd".to_string());
    }
    let mut usbmuxd = usbmuxd.unwrap();

    let devs = usbmuxd.get_devices().await.unwrap();
    if devs.is_empty() {
        return Ok(vec![]);
    }

    let device_info_futures: Vec<_> = devs
        .iter()
        .map(|d| async move {
            let provider = d.to_provider(UsbmuxdAddr::from_env_var().unwrap(), "y-code");
            let device_uid = d.device_id;

            let mut lockdown_client = match LockdownClient::connect(&provider).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Unable to connect to lockdown: {e:?}");
                    return DeviceInfo {
                        name: String::from("Unknown Device"),
                        id: device_uid,
                        uuid: d.udid.clone(),
                    };
                }
            };

            let device_name = lockdown_client
                .get_value("DeviceName", None)
                .await
                .expect("Failed to get device name")
                .as_string()
                .expect("Failed to convert device name to string")
                .to_string();

            DeviceInfo {
                name: device_name,
                id: device_uid,
                uuid: d.udid.clone(),
            }
        })
        .collect();

    Ok(futures::future::join_all(device_info_futures).await)
}

pub async fn install_app(
    device: &DeviceInfo,
    app_path: &Path,
    callback: impl Fn(u64) -> (),
) -> Result<(), Error> {
    let mut usbmuxd = UsbmuxdConnection::default()
        .await
        .map_err(|e| Error::IdeviceError(e))?;
    let device = usbmuxd
        .get_device(&device.uuid)
        .await
        .map_err(|e| Error::IdeviceError(e))?;

    let provider = device.to_provider(UsbmuxdAddr::from_env_var().unwrap(), "y-code");

    let mut afc_client = AfcClient::connect(&provider)
        .await
        .map_err(|e| Error::IdeviceError(e))?;

    let dir = format!(
        "PublicStaging/{}",
        app_path.file_name().unwrap().to_string_lossy()
    );
    afc_upload_dir(&mut afc_client, app_path, &dir).await?;

    let mut instproxy_client = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| Error::IdeviceError(e))?;

    let mut options = plist::Dictionary::new();
    options.insert("PackageType".to_string(), "Developer".into());
    instproxy_client
        .install_with_callback(
            dir,
            Some(plist::Value::Dictionary(options)),
            async |(percentage, _)| {
                callback(percentage);
            },
            (),
        )
        .await
        .map_err(|e| Error::IdeviceError(e))?;

    Ok(())
}

fn afc_upload_dir<'a>(
    afc_client: &'a mut AfcClient,
    path: &'a Path,
    afc_path: &'a str,
) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
    Box::pin(async move {
        let entries = std::fs::read_dir(path).map_err(|e| Error::Filesystem(e))?;
        afc_client
            .mk_dir(afc_path)
            .await
            .map_err(|e| Error::IdeviceError(e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::Filesystem(e))?;
            let path = entry.path();
            if path.is_dir() {
                let new_afc_path = format!(
                    "{}/{}",
                    afc_path,
                    path.file_name().unwrap().to_string_lossy()
                );
                afc_upload_dir(afc_client, &path, &new_afc_path).await?;
            } else {
                let mut file_handle = afc_client
                    .open(
                        format!(
                            "{}/{}",
                            afc_path,
                            path.file_name().unwrap().to_string_lossy()
                        ),
                        idevice::afc::opcode::AfcFopenMode::WrOnly,
                    )
                    .await
                    .map_err(|e| Error::IdeviceError(e))?;
                let bytes = std::fs::read(&path).map_err(|e| Error::Filesystem(e))?;
                file_handle
                    .write(&bytes)
                    .await
                    .map_err(|e| Error::IdeviceError(e))?;
            }
        }
        Ok(())
    })
}
