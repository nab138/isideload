use rootcause::prelude::*;
use tracing::{error, info};

use crate::{
    dev::{
        certificates::CertificatesApi, developer_session::DeveloperSession, teams::DeveloperTeam,
    },
    util::storage::SideloadingStorage,
};

pub struct CertificateIdentity {
    pub machine_id: String,
    pub machine_name: String,
}

impl CertificateIdentity {
    pub async fn retrieve(
        machine_name: &str,
        developer_session: DeveloperSession,
        team: &DeveloperTeam,
        storage: &dyn SideloadingStorage,
    ) -> Result<Self, Report> {
        let stored =
            Self::retrieve_from_storage(machine_name, developer_session, team, storage).await;
        if let Ok(Some(cert)) = stored {
            return Ok(cert);
        }

        if let Err(e) = stored {
            error!("Failed to load certificate from storage: {:?}", e);
        } else {
            info!("No stored certificate found, generating");
        }

        todo!("generate CSR")
    }

    async fn retrieve_from_storage(
        machine_name: &str,
        developer_session: DeveloperSession,
        team: &DeveloperTeam,
        storage: &dyn SideloadingStorage,
    ) -> Result<Option<Self>, Report> {
        let cert = storage.retrieve_data("cert")?;
        if cert.is_none() {
            return Ok(None);
        }
        let cert = cert.unwrap();
        let private_key = storage.retrieve_data("key")?;
        if private_key.is_none() {
            return Ok(None);
        }

        for cert in developer_session
            .list_ios_certs(team)
            .await?
            .iter()
            .filter(|c| c.machine_name.unwrap_or_default() == machine_name)
        {}

        Ok(())
    }
}
