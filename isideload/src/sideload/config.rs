use std::fmt::Display;

use crate::dev::teams::DeveloperTeam;

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

pub struct SideloadConfiguration {
    pub team_selection: TeamSelection,
}

impl Default for SideloadConfiguration {
    fn default() -> Self {
        SideloadConfiguration {
            team_selection: TeamSelection::First,
        }
    }
}

impl SideloadConfiguration {
    pub fn builder() -> Self {
        Self::default()
    }

    pub fn team_selection(mut self, selection: TeamSelection) -> Self {
        self.team_selection = selection;
        self
    }
}
