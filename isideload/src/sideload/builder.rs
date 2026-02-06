use std::fmt::Display;

use crate::{dev::teams::DeveloperTeam, util::storage::SideloadingStorage};

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

pub struct SideloaderBuilder {
    pub team_selection: TeamSelection,
    pub storage: Box<dyn SideloadingStorage>,
}

impl Default for SideloaderBuilder {
    fn default() -> Self {
        SideloaderBuilder {
            team_selection: TeamSelection::First,
            storage: Box::new(crate::util::storage::new_storage()),
        }
    }
}

impl SideloaderBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn team_selection(mut self, selection: TeamSelection) -> Self {
        self.team_selection = selection;
        self
    }

    pub fn storage(mut self, storage: Box<dyn SideloadingStorage>) -> Self {
        self.storage = storage;
        self
    }
}
