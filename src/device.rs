use idevice::{
    IdeviceService,
    afc::AfcClient,
    installation_proxy::InstallationProxyClient,
    lockdown::LockdownClient,
    usbmuxd::{UsbmuxdAddr, UsbmuxdConnection},
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

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
    app_path: &PathBuf,
    callback: impl Fn(u64) -> (),
) -> Result<(), String> {
    let mut usbmuxd = UsbmuxdConnection::default()
        .await
        .map_err(|e| format!("Failed to connect to usbmuxd: {:?}", e))?;
    let device = usbmuxd
        .get_device(&device.uuid)
        .await
        .map_err(|e| format!("Failed to get device: {:?}", e))?;

    let provider = device.to_provider(UsbmuxdAddr::from_env_var().unwrap(), "y-code");

    let mut afc_client = AfcClient::connect(&provider)
        .await
        .map_err(|e| format!("Failed to connect to AFC: {:?}", e))?;

    let dir = format!(
        "PublicStaging/{}",
        app_path.file_name().unwrap().to_string_lossy()
    );
    afc_upload_dir(&mut afc_client, app_path, &dir)
        .await
        .map_err(|e| format!("Failed to upload directory: {:?}", e))?;

    let mut instproxy_client = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("Failed to connect to installation proxy: {:?}", e))?;

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
        .map_err(|e| format!("Failed to install app: {:?}", e))?;

    Ok(())
}

fn afc_upload_dir<'a>(
    afc_client: &'a mut AfcClient,
    path: &'a PathBuf,
    afc_path: &'a str,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        let entries =
            std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?;
        afc_client
            .mk_dir(afc_path)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
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
                    .map_err(|e| format!("Failed to open file: {}", e))?;
                let bytes =
                    std::fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
                file_handle
                    .write(&bytes)
                    .await
                    .map_err(|e| format!("Failed to write file: {}", e))?;
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn refresh_idevice(window: tauri::Window) {
    match list_devices().await {
        Ok(devices) => {
            window
                .emit("idevices", devices)
                .expect("Failed to send devices");
        }
        Err(e) => {
            window
                .emit("idevices", Vec::<DeviceInfo>::new())
                .expect("Failed to send error");
            eprintln!("Failed to list devices: {}", e);
        }
    };
}
