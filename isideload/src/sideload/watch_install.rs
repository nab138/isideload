use idevice::{
    Idevice, IdeviceService,
    pairing_file::PairingFile,
    provider::IdeviceProvider,
    services::{
        companion_proxy::CompanionProxy, installation_proxy::InstallationProxyClient,
        lockdown::LockdownClient,
    },
};
use plist::{Dictionary, Value};
use rootcause::{option_ext::OptionExt, prelude::*};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{info, warn};

use crate::{SideloadError as Error, sideload::bundle::Bundle};

const WATCH_LOCKDOWN_PORT: u16 = 62078;
const WATCH_LOCKDOWN_SERVICE: &str = "com.apple.mobile.lockdownd";
const WATCH_INSTALL_PROXY_SERVICE: &str = "com.apple.mobile.installation_proxy";
const WATCH_ZIP_SERVICE: &str = "com.apple.streaming_zip_conduit";
const WATCH_FORWARD_CONNECT_ATTEMPTS: usize = 20;
const WATCH_FORWARD_CONNECT_DELAY_MS: u64 = 50;
const CENTRAL_DIRECTORY_HEADER: &[u8] = &[0x50, 0x4b, 0x01, 0x02];
const ZIP_EXTRA: &[u8] = &[
    0x55, 0x54, 0x0d, 0x00, 0x07, 0xf3, 0xa2, 0xec, 0x60, 0xf6, 0xa2, 0xec, 0x60, 0xf3,
    0xa2, 0xec, 0x60, 0x75, 0x78, 0x0b, 0x00, 0x01, 0x04, 0xf5, 0x01, 0x00, 0x00, 0x04,
    0x14, 0x00, 0x00, 0x00,
];

/// Install already-signed embedded Watch apps directly on the paired Apple Watch.
///
/// iPhone installation can leave an embedded Watch app as a process-scoped placeholder.
/// watchOS exposes `streaming_zip_conduit`, which accepts the signed Watch `.app` directly
/// and finalizes it as a real installed application. Before streaming, we best-effort remove
/// any existing placeholder/previous installation for the same Watch bundle identifier.
pub async fn install_watch_apps(
    device_provider: &impl IdeviceProvider,
    watch_apps: &[Bundle],
    host_name: &str,
    progress_callback: impl Fn(u64) + Send + Sync,
) -> Result<(), Report> {
    if watch_apps.is_empty() {
        return Ok(());
    }

    let iphone_pairing = device_provider
        .get_pairing_file()
        .await
        .map_err(Error::IdeviceError)?;

    let forwarded_lockdown_port = {
        let mut companion_proxy = CompanionProxy::connect(device_provider)
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to connect to fresh Apple Watch companion proxy for lockdown forwarding")?;

        companion_proxy
            .start_forwarding_service_port(
                WATCH_LOCKDOWN_PORT,
                Some(WATCH_LOCKDOWN_SERVICE),
                None,
            )
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to forward Apple Watch lockdown")?
    };

    let result = async {
        let watch_connection = connect_forwarded_watch_port(
            device_provider,
            forwarded_lockdown_port,
            "lockdown",
        )
        .await?;
        let mut watch_lockdown = LockdownClient::new(watch_connection);

        let watch_pairing = watch_lockdown
            .pair(
                iphone_pairing.host_id.clone(),
                iphone_pairing.system_buid.clone(),
                Some(host_name),
            )
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to pair with the Apple Watch through companion proxy")?;

        let legacy = watch_lockdown
            .start_session(&watch_pairing)
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to start Apple Watch lockdown session")?;

        for watch_app in watch_apps {
            let bundle_id = watch_app
                .bundle_identifier()
                .ok_or_report()
                .context("Watch app is missing CFBundleIdentifier")?;

            remove_existing_watch_app(
                device_provider,
                &mut watch_lockdown,
                &watch_pairing,
                legacy,
                bundle_id,
            )
            .await?;

            install_watch_app_zip_conduit(
                device_provider,
                &mut watch_lockdown,
                &watch_pairing,
                legacy,
                watch_app,
                &progress_callback,
            )
            .await?;
        }

        Ok(())
    }
    .await;

    match CompanionProxy::connect(device_provider).await {
        Ok(mut companion_proxy) => {
            if let Err(e) = companion_proxy
                .stop_forwarding_service_port(WATCH_LOCKDOWN_PORT)
                .await
            {
                warn!("Failed to stop Apple Watch lockdown forwarding: {e}");
            }
        }
        Err(e) => {
            warn!("Failed to open fresh Apple Watch companion proxy to stop lockdown forwarding: {e}");
        }
    }

    result
}

async fn connect_forwarded_watch_port(
    device_provider: &impl IdeviceProvider,
    forwarded_port: u16,
    service: &str,
) -> Result<Idevice, Report> {
    for attempt in 1..=WATCH_FORWARD_CONNECT_ATTEMPTS {
        match device_provider.connect(forwarded_port).await {
            Ok(connection) => {
                if attempt > 1 {
                    info!(
                        "Connected to Apple Watch {} on forwarded port {} after {} attempts",
                        service, forwarded_port, attempt
                    );
                }
                return Ok(connection);
            }
            Err(error) if attempt < WATCH_FORWARD_CONNECT_ATTEMPTS => {
                warn!(
                    "Apple Watch {} forwarded port {} is not ready (attempt {}/{}): {}",
                    service,
                    forwarded_port,
                    attempt,
                    WATCH_FORWARD_CONNECT_ATTEMPTS,
                    error
                );
                tokio::time::sleep(Duration::from_millis(
                    WATCH_FORWARD_CONNECT_DELAY_MS,
                ))
                .await;
            }
            Err(error) => {
                bail!(
                    "Failed to connect to Apple Watch {} on forwarded iPhone port {} after {} attempts: {}",
                    service,
                    forwarded_port,
                    WATCH_FORWARD_CONNECT_ATTEMPTS,
                    error
                );
            }
        }
    }

    unreachable!("bounded forwarded-port retry loop must return")
}
async fn remove_existing_watch_app(
    device_provider: &impl IdeviceProvider,
    watch_lockdown: &mut LockdownClient,
    watch_pairing: &PairingFile,
    legacy: bool,
    bundle_id: &str,
) -> Result<(), Report> {
    let (remote_port, ssl) = watch_lockdown
        .start_service(WATCH_INSTALL_PROXY_SERVICE)
        .await
        .map_err(Error::IdeviceError)
        .context("Failed to start Apple Watch installation proxy")?;

    let forwarded_port = {
        let mut companion_proxy = CompanionProxy::connect(device_provider)
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to connect to fresh Apple Watch companion proxy for installation proxy forwarding")?;

        companion_proxy
            .start_forwarding_service_port(
                remote_port,
                Some(WATCH_INSTALL_PROXY_SERVICE),
                None,
            )
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to forward Apple Watch installation proxy")?
    };

    let result = async {
        let mut connection = connect_forwarded_watch_port(
            device_provider,
            forwarded_port,
            "installation proxy",
        )
        .await?;

        if ssl {
            connection
                .start_session(watch_pairing, legacy)
                .await
                .map_err(Error::IdeviceError)
                .context("Failed to secure Apple Watch installation proxy connection")?;
        }

        let mut install_proxy = InstallationProxyClient::new(connection);
        match install_proxy.uninstall(bundle_id, None).await {
            Ok(()) => info!("Removed existing Apple Watch app/placeholder: {bundle_id}"),
            Err(e) => {
                // A missing bundle is expected on a first installation. If a stale coordinator
                // still exists, zip_conduit will return the real watchOS error below.
                info!("No removable Apple Watch app/placeholder for {bundle_id}: {e}");
            }
        }

        Ok(())
    }
    .await;

    match CompanionProxy::connect(device_provider).await {
        Ok(mut companion_proxy) => {
            if let Err(e) = companion_proxy.stop_forwarding_service_port(remote_port).await {
                warn!("Failed to stop Apple Watch installation proxy forwarding: {e}");
            }
        }
        Err(e) => {
            warn!("Failed to open fresh Apple Watch companion proxy to stop installation proxy forwarding: {e}");
        }
    }

    result
}

async fn install_watch_app_zip_conduit(
    device_provider: &impl IdeviceProvider,
    watch_lockdown: &mut LockdownClient,
    watch_pairing: &PairingFile,
    legacy: bool,
    watch_app: &Bundle,
    progress_callback: &(impl Fn(u64) + Send + Sync),
) -> Result<(), Report> {
    let (remote_port, ssl) = watch_lockdown
        .start_service(WATCH_ZIP_SERVICE)
        .await
        .map_err(Error::IdeviceError)
        .context("Failed to start Apple Watch streaming_zip_conduit")?;

    let forwarded_port = {
        let mut companion_proxy = CompanionProxy::connect(device_provider)
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to connect to fresh Apple Watch companion proxy for streaming_zip_conduit forwarding")?;

        companion_proxy
            .start_forwarding_service_port(
                remote_port,
                Some(WATCH_ZIP_SERVICE),
                None,
            )
            .await
            .map_err(Error::IdeviceError)
            .context("Failed to forward Apple Watch streaming_zip_conduit")?
    };

    let result = async {
        let mut connection = connect_forwarded_watch_port(
            device_provider,
            forwarded_port,
            "streaming_zip_conduit",
        )
        .await?;

        if ssl {
            connection
                .start_session(watch_pairing, legacy)
                .await
                .map_err(Error::IdeviceError)
                .context("Failed to secure Apple Watch streaming_zip_conduit connection")?;
        }

        stream_watch_app(&mut connection, watch_app, progress_callback).await
    }
    .await;

    match CompanionProxy::connect(device_provider).await {
        Ok(mut companion_proxy) => {
            if let Err(e) = companion_proxy.stop_forwarding_service_port(remote_port).await {
                warn!("Failed to stop Apple Watch streaming_zip_conduit forwarding: {e}");
            }
        }
        Err(e) => {
            warn!("Failed to open fresh Apple Watch companion proxy to stop streaming_zip_conduit forwarding: {e}");
        }
    }

    result
}

async fn stream_watch_app(
    connection: &mut Idevice,
    watch_app: &Bundle,
    progress_callback: &(impl Fn(u64) + Send + Sync),
) -> Result<(), Report> {
    let app_name = watch_app
        .bundle_dir
        .file_name()
        .ok_or_report()
        .context("Watch app path has no file name")?
        .to_string_lossy()
        .to_string();

    let mut files = Vec::new();
    collect_files(&watch_app.bundle_dir, &mut files)?;
    files.sort();

    let mut entries = Vec::with_capacity(files.len());
    let mut total_uncompressed = 0u64;

    for source in files {
        let relative = source
            .strip_prefix(&watch_app.bundle_dir)
            .context("Failed to create relative Watch app path")?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let destination = format!("Payload/{app_name}/{relative}");
        let size = isideload_vfs::fs::metadata(&source)?.len();
        total_uncompressed = total_uncompressed.saturating_add(size);
        entries.push((source, destination));
    }

    let media_subdir = format!("PublicStaging/{app_name}.ipa");
    let mut install_options = Dictionary::new();
    insert_integer(&mut install_options, "DisableDeltaTransfer", 1);
    insert_string(
        &mut install_options,
        "InstallDeltaTypeKey",
        "InstallDeltaTypeSparseIPAFiles",
    );
    insert_integer(&mut install_options, "IsUserInitiated", 1);
    insert_string(&mut install_options, "PackageType", "Customer");
    insert_integer(&mut install_options, "PreferWifi", 1);

    let mut init_transfer = Dictionary::new();
    init_transfer.insert(
        "InstallOptionsDictionary".to_string(),
        Value::Dictionary(install_options),
    );
    insert_integer(&mut init_transfer, "InstallTransferredDirectory", 1);
    insert_string(&mut init_transfer, "MediaSubdir", media_subdir);
    insert_integer(&mut init_transfer, "UserInitiatedTransfer", 0);

    send_prefixed_plist(connection, init_transfer).await?;

    let mut metadata = Dictionary::new();
    insert_integer(&mut metadata, "StandardDirectoryPerms", 16877);
    insert_integer(&mut metadata, "StandardFilePerms", -32348);
    insert_integer(
        &mut metadata,
        "RecordCount",
        i64::try_from(entries.len() + 2).context("Too many Watch app files")?,
    );
    insert_integer(
        &mut metadata,
        "TotalUncompressedBytes",
        i64::try_from(total_uncompressed).context("Watch app is too large")?,
    );
    insert_integer(&mut metadata, "Version", 2);

    let mut metadata_bytes = Vec::new();
    Value::Dictionary(metadata).to_writer_xml(&mut metadata_bytes)?;

    send_zip_directory(connection, "META-INF/").await?;
    send_zip_file(
        connection,
        "META-INF/com.apple.ZipMetadata.plist",
        &metadata_bytes,
    )
    .await?;

    for (source, destination) in entries {
        let data = isideload_vfs::fs::read(&source)?;
        send_zip_file(connection, &destination, &data).await?;
    }

    connection
        .send_raw(CENTRAL_DIRECTORY_HEADER)
        .await
        .map_err(Error::IdeviceError)?;

    wait_for_watch_install(connection, progress_callback).await
}

async fn wait_for_watch_install(
    connection: &mut Idevice,
    progress_callback: &(impl Fn(u64) + Send + Sync),
) -> Result<(), Report> {
    loop {
        let response = read_prefixed_plist(connection).await?;

        if response
            .get("Status")
            .and_then(Value::as_string)
            .is_some_and(|status| status == "DataComplete")
        {
            progress_callback(100);
            info!("Apple Watch installation completed");
            return Ok(());
        }

        let progress = response
            .get("InstallProgressDict")
            .and_then(Value::as_dictionary)
            .ok_or_else(|| report!("Unexpected Apple Watch install response: {response:?}"))?;

        if let Some(error) = progress.get("Error").and_then(Value::as_string) {
            let description = progress
                .get("ErrorDescription")
                .and_then(Value::as_string)
                .unwrap_or("No ErrorDescription returned by watchOS");
            bail!("Apple Watch installation failed: {error}: {description}");
        }

        let percent = progress
            .get("PercentComplete")
            .and_then(Value::as_unsigned_integer)
            .unwrap_or(0);
        let status = progress
            .get("Status")
            .and_then(Value::as_string)
            .unwrap_or("Unknown");

        info!("Installing Apple Watch app: {percent}% {status}");
        progress_callback(percent);
    }
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Report> {
    for entry in isideload_vfs::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = isideload_vfs::fs::metadata(&path)?;

        if metadata.is_dir() {
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }

    Ok(())
}

async fn send_prefixed_plist(
    connection: &mut Idevice,
    dictionary: Dictionary,
) -> Result<(), Report> {
    let mut payload = Vec::new();
    Value::Dictionary(dictionary).to_writer_xml(&mut payload)?;
    let payload_len = u32::try_from(payload.len()).context("Watch plist is too large")?;

    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(&payload);

    connection
        .send_raw(&frame)
        .await
        .map_err(Error::IdeviceError)?;
    Ok(())
}

async fn read_prefixed_plist(connection: &mut Idevice) -> Result<Dictionary, Report> {
    let len = connection
        .read_raw(4)
        .await
        .map_err(Error::IdeviceError)?;
    let payload_len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
    let payload = connection
        .read_raw(payload_len)
        .await
        .map_err(Error::IdeviceError)?;
    let value: Value = plist::from_bytes(&payload)?;

    match value {
        Value::Dictionary(dictionary) => Ok(dictionary),
        other => bail!("Expected dictionary plist from Apple Watch, got {other:?}"),
    }
}

async fn send_zip_directory(connection: &mut Idevice, name: &str) -> Result<(), Report> {
    let header = zip_header(name, 0, 0)?;
    connection
        .send_raw(&header)
        .await
        .map_err(Error::IdeviceError)?;
    Ok(())
}

async fn send_zip_file(connection: &mut Idevice, name: &str, data: &[u8]) -> Result<(), Report> {
    let size = u32::try_from(data.len()).context("Watch app file is too large")?;
    let header = zip_header(name, size, crc32_ieee(data))?;

    connection
        .send_raw(&header)
        .await
        .map_err(Error::IdeviceError)?;
    connection
        .send_raw(data)
        .await
        .map_err(Error::IdeviceError)?;
    Ok(())
}

fn zip_header(name: &str, size: u32, crc32: u32) -> Result<Vec<u8>, Report> {
    let name_bytes = name.as_bytes();
    let name_len = u16::try_from(name_bytes.len()).context("Watch zip path is too long")?;
    let extra_len = u16::try_from(ZIP_EXTRA.len()).context("Watch zip extra data is too large")?;

    let mut header = Vec::with_capacity(30 + name_bytes.len() + ZIP_EXTRA.len());
    push_u32_le(&mut header, 0x04034b50);
    push_u16_le(&mut header, 20);
    push_u16_le(&mut header, 0);
    push_u16_le(&mut header, 0);
    push_u16_le(&mut header, 0xbdef);
    push_u16_le(&mut header, 0x52ec);
    push_u32_le(&mut header, crc32);
    push_u32_le(&mut header, size);
    push_u32_le(&mut header, size);
    push_u16_le(&mut header, name_len);
    push_u16_le(&mut header, extra_len);
    header.extend_from_slice(name_bytes);
    header.extend_from_slice(ZIP_EXTRA);
    Ok(header)
}

fn push_u16_le(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn insert_string(dictionary: &mut Dictionary, key: &str, value: impl Into<String>) {
    dictionary.insert(key.to_string(), Value::String(value.into()));
}

fn insert_integer(dictionary: &mut Dictionary, key: &str, value: i64) {
    dictionary.insert(key.to_string(), Value::Integer(value.into()));
}

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;

    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }

    !crc
}
