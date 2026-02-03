use std::path::PathBuf;

use idevice::provider::IdeviceProvider;
use rootcause::prelude::*;
use tracing::info;

use crate::dev::teams::TeamsApi;
use crate::dev::{developer_session::DeveloperSession, devices::DevicesApi};
use crate::util::device::IdeviceInfo;

pub mod config;
pub use config::{SideloadConfiguration, TeamSelection};

pub async fn sideload_app(
    device_provider: &impl IdeviceProvider,
    dev_session: &mut DeveloperSession,
    app_path: PathBuf,
    config: &SideloadConfiguration,
) -> Result<(), Report> {
    let device_info = IdeviceInfo::from_device(device_provider).await?;

    let teams = dev_session.list_teams().await?;
    let team = match teams.len() {
        0 => {
            bail!("No developer teams available")
        }
        1 => &teams[0],
        _ => {
            info!(
                "Multiple developer teams found, {} as per configuration",
                config.team_selection
            );
            match &config.team_selection {
                TeamSelection::First => &teams[0],
                TeamSelection::Prompt(prompt_fn) => {
                    let selection = prompt_fn(&teams).ok_or_else(|| report!("No team selected"))?;
                    teams
                        .iter()
                        .find(|t| t.team_id == selection)
                        .ok_or_else(|| report!("No team found with ID {}", selection))?
                }
            }
        }
    };

    dev_session
        .ensure_device_registered(team, &device_info.name, &device_info.udid, None)
        .await?;

    Ok(())
}
