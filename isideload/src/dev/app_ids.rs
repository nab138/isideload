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
pub struct AppId {
    pub app_id_id: String,
    pub identifier: String,
    pub name: String,
    pub features: Option<Dictionary>,
    pub expiration_date: Option<Date>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppIdsResponse {
    pub app_ids: Vec<AppId>,
    pub max_quantity: Option<u64>,
    pub available_quantity: Option<u64>,
}

#[async_trait::async_trait]
pub trait AppIdsApi {
    fn developer_session(&self) -> &DeveloperSession<'_>;

    async fn add_app_id(
        &self,
        team: &DeveloperTeam,
        name: &str,
        identifier: &str,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<AppId, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "identifier": identifier,
            "name": name,
        });

        let app_id: AppId = self
            .developer_session()
            .send_dev_request(&dev_url("addAppId", device_type), body, "appId")
            .await
            .context("Failed to add developer app ID")?;

        Ok(app_id)
    }

    async fn list_app_ids(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<ListAppIdsResponse, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let response: Value = self
            .developer_session()
            .send_dev_request_no_response(&dev_url("listAppIds", device_type), body)
            .await
            .context("Failed to list developer app IDs")?
            .into();

        let app_ids: ListAppIdsResponse = plist::from_value(&response).map_err(|e| {
            report!("Failed to deserialize app id response: {:?}", e).attach(
                SensitivePlistAttachment::new(
                    response
                        .as_dictionary()
                        .unwrap_or(&Dictionary::new())
                        .clone(),
                ),
            )
        })?;

        Ok(app_ids)
    }
}

impl AppIdsApi for DeveloperSession<'_> {
    fn developer_session(&self) -> &DeveloperSession<'_> {
        self
    }
}
