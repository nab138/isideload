use std::fmt::Display;

use crate::{
    dev::{
        certificates::DevelopmentCertificate, developer_session::DeveloperSession,
        teams::DeveloperTeam,
    },
    sideload::sideloader::Sideloader,
    util::storage::SideloadingStorage,
};

/// Configuration for selecting a developer team during sideloading
///
/// If there is only one team, it will be selected automatically regardless of this setting.
/// If there are multiple teams, the behavior will depend on this setting.
pub enum TeamSelection {
    /// Select the first team automatically
    First,
    /// Prompt the user to select a team
    Prompt(fn(&Vec<DeveloperTeam>) -> Option<String>),
}

impl Display for TeamSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamSelection::First => write!(f, "first team"),
            TeamSelection::Prompt(_) => write!(f, "prompting for team"),
        }
    }
}

pub enum MaxCertsBehavior {
    /// If the maximum number of certificates is reached, revoke certs until it is possible to create a new certificate
    Revoke,
    /// If the maximum number of certificates is reached, return an error instead of creating a new certificate
    Error,
    /// If the maximum number of certificates is reached, prompt the user to select which certificates to revoke until it is possible to create a new certificate
    Prompt(fn(&Vec<DevelopmentCertificate>) -> Option<Vec<DevelopmentCertificate>>),
}

pub struct SideloaderBuilder {
    developer_session: DeveloperSession,
    apple_email: String,
    team_selection: Option<TeamSelection>,
    max_certs_behavior: Option<MaxCertsBehavior>,
    storage: Option<Box<dyn SideloadingStorage>>,
    machine_name: Option<String>,
}

impl SideloaderBuilder {
    pub fn new(developer_session: DeveloperSession, apple_email: String) -> Self {
        SideloaderBuilder {
            team_selection: None,
            storage: None,
            developer_session,
            machine_name: None,
            apple_email,
            max_certs_behavior: None,
        }
    }

    pub fn team_selection(mut self, selection: TeamSelection) -> Self {
        self.team_selection = Some(selection);
        self
    }

    pub fn storage(mut self, storage: Box<dyn SideloadingStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn machine_name(mut self, machine_name: String) -> Self {
        self.machine_name = Some(machine_name);
        self
    }

    pub fn max_certs_behavior(mut self, behavior: MaxCertsBehavior) -> Self {
        self.max_certs_behavior = Some(behavior);
        self
    }

    pub fn build(self) -> Sideloader {
        Sideloader::new(
            self.developer_session,
            self.apple_email,
            self.team_selection.unwrap_or(TeamSelection::First),
            self.max_certs_behavior.unwrap_or(MaxCertsBehavior::Error),
            self.machine_name.unwrap_or_else(|| "isideload".to_string()),
            self.storage
                .unwrap_or_else(|| Box::new(crate::util::storage::new_storage())),
        )
    }
}
