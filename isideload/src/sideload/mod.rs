use std::path::PathBuf;

use idevice::provider::IdeviceProvider;
use rootcause::prelude::*;

use crate::dev::developer_session::DeveloperSession;
use crate::util::device::IdeviceInfo;

pub async fn sideload_app(
    device_provider: &impl IdeviceProvider,
    dev_session: &DeveloperSession,
    app_path: PathBuf,
) -> Result<(), Report> {
    let device_info = IdeviceInfo::from_device(device_provider).await?;
    Ok(())
}
