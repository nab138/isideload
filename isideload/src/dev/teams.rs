use crate::dev::{
    developer_session::DeveloperSession,
    device_type::{DeveloperDeviceType::*, dev_url},
};
use rootcause::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperTeam {
    pub name: Option<String>,
    pub team_id: String,
    pub r#type: Option<String>,
    pub status: Option<String>,
}

#[async_trait::async_trait]
pub trait TeamsApi {
    fn developer_session(&self) -> &DeveloperSession<'_>;

    async fn list_teams(&self) -> Result<Vec<DeveloperTeam>, Report> {
        let response: Vec<DeveloperTeam> = self
            .developer_session()
            .send_dev_request(&dev_url("listTeams", Any), None, "teams")
            .await
            .context("Failed to list developer teams")?;

        Ok(response)
    }
}

impl TeamsApi for DeveloperSession<'_> {
    fn developer_session(&self) -> &DeveloperSession<'_> {
        self
    }
}
