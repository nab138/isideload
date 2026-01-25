pub mod remote_v3;

use crate::auth::grandslam::GrandSlam;
use rootcause::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub struct AnisetteClientInfo {
    pub client_info: String,
    pub user_agent: String,
}

#[async_trait::async_trait]
pub trait AnisetteProvider {
    async fn get_anisette_headers(
        &mut self,
        gs: &mut GrandSlam,
    ) -> Result<HashMap<String, String>, Report>;

    async fn get_client_info(&mut self) -> Result<AnisetteClientInfo, Report>;
}
