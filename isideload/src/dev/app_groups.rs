use crate::{
    dev::{
        developer_session::DeveloperSession,
        device_type::{DeveloperDeviceType, dev_url},
        teams::DeveloperTeam,
    },
    util::plist::SensitivePlistAttachment,
};
use plist::{Date, Dictionary, Value};
use plist_macro::plist;
use rootcause::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppGroup {
    pub name: Option<String>,
    pub identifier: String,
    pub application_group: String,
}

#[async_trait::async_trait]
pub trait AppGroupsApi {
    fn developer_session(&self) -> &DeveloperSession<'_>;

    async fn list_app_groups(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Vec<AppGroup>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let app_groups: Vec<AppGroup> = self
            .developer_session()
            .send_dev_request(
                &dev_url("listApplicationGroups", device_type),
                body,
                "applicationGroupList",
            )
            .await
            .context("Failed to list developer app groups")?;

        Ok(app_groups)
    }

    async fn add_app_group(
        &self,
        team: &DeveloperTeam,
        name: &str,
        identifier: &str,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<AppGroup, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "name": name,
            "identifier": identifier,
        });

        let app_group: AppGroup = self
            .developer_session()
            .send_dev_request(
                &dev_url("addApplicationGroup", device_type),
                body,
                "applicationGroup",
            )
            .await
            .context("Failed to add developer app group")?;

        Ok(app_group)
    }
}

impl AppGroupsApi for DeveloperSession<'_> {
    fn developer_session(&self) -> &DeveloperSession<'_> {
        self
    }
}
