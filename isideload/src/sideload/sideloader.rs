use crate::{
    dev::{
        developer_session::DeveloperSession,
        devices::DevicesApi,
        teams::{DeveloperTeam, TeamsApi},
    },
    sideload::TeamSelection,
    util::{device::IdeviceInfo, storage::SideloadingStorage},
};

use std::path::PathBuf;

use idevice::provider::IdeviceProvider;
use rootcause::prelude::*;
use tracing::info;

pub struct Sideloader {
    team_selection: TeamSelection,
    storage: Box<dyn SideloadingStorage>,
    dev_session: DeveloperSession,
}

impl Sideloader {
    pub fn new(
        team_selection: TeamSelection,
        storage: Box<dyn SideloadingStorage>,
        dev_session: DeveloperSession,
    ) -> Self {
        Sideloader {
            team_selection,
            storage,
            dev_session,
        }
    }

    pub async fn install_app(
        &mut self,
        device_provider: &impl IdeviceProvider,
        app_path: PathBuf,
    ) -> Result<(), Report> {
        let device_info = IdeviceInfo::from_device(device_provider).await?;

        let team = self.get_team().await?;

        self.dev_session
            .ensure_device_registered(&team, &device_info.name, &device_info.udid, None)
            .await?;

        Ok(())
    }

    pub async fn get_team(&mut self) -> Result<DeveloperTeam, Report> {
        let teams = self.dev_session.list_teams().await?;
        Ok(match teams.len() {
            0 => {
                bail!("No developer teams available")
            }
            1 => teams.into_iter().next().unwrap(),
            _ => {
                info!(
                    "Multiple developer teams found, {} as per configuration",
                    self.team_selection
                );
                match &self.team_selection {
                    TeamSelection::First => teams.into_iter().next().unwrap(),
                    TeamSelection::Prompt(prompt_fn) => {
                        let selection =
                            prompt_fn(&teams).ok_or_else(|| report!("No team selected"))?;
                        teams
                            .into_iter()
                            .find(|t| t.team_id == selection)
                            .ok_or_else(|| report!("No team found with ID {}", selection))?
                    }
                }
            }
        })
    }
}
