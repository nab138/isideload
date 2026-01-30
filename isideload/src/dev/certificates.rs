use crate::dev::{
    developer_session::DeveloperSession,
    device_type::{DeveloperDeviceType, dev_url},
    teams::DeveloperTeam,
};
use plist_macro::plist;
use rootcause::prelude::*;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentCertificate {
    pub name: Option<String>,
    pub certificate_id: Option<String>,
    pub serial_number: Option<String>,
    pub machine_id: Option<String>,
    pub cert_content: Option<ByteBuf>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CertRequest {
    pub cert_request_id: String,
}

// the automatic debug implementation spams the console with the cert content bytes
impl std::fmt::Debug for DevelopmentCertificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DevelopmentCertificate")
            .field("name", &self.name)
            .field("certificate_id", &self.certificate_id)
            .field("serial_number", &self.serial_number)
            .field("machine_id", &self.machine_id)
            .field(
                "cert_content",
                &self
                    .cert_content
                    .as_ref()
                    .map(|c| format!("Some([{} bytes])", c.len()))
                    .unwrap_or("None".to_string()),
            )
            .finish()
    }
}

#[async_trait::async_trait]
pub trait CertificatesApi {
    fn developer_session(&self) -> &DeveloperSession<'_>;

    async fn list_all_development_certs(
        &self,
        team: &DeveloperTeam,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<Vec<DevelopmentCertificate>, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
        });

        let certs: Vec<DevelopmentCertificate> = self
            .developer_session()
            .send_dev_request(
                &dev_url("listAllDevelopmentCerts", device_type),
                body,
                "certificates",
            )
            .await
            .context("Failed to list development certificates")?;

        Ok(certs)
    }

    async fn revoke_development_cert(
        &self,
        team: &DeveloperTeam,
        serial_number: &str,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<(), Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "serialNumber": serial_number,
        });

        self.developer_session()
            .send_dev_request_no_response(
                &dev_url("revokeDevelopmentCert", device_type),
                Some(body),
            )
            .await
            .context("Failed to revoke development certificate")?;

        Ok(())
    }

    async fn submit_development_csr(
        &self,
        team: &DeveloperTeam,
        csr_content: String,
        machine_name: String,
        device_type: impl Into<Option<DeveloperDeviceType>> + Send,
    ) -> Result<CertRequest, Report> {
        let body = plist!(dict {
            "teamId": &team.team_id,
            "csrContent": csr_content,
            "machineName": machine_name,
            "machineId": Uuid::new_v4().to_string().to_uppercase(),
        });

        let cert: CertRequest = self
            .developer_session()
            .send_dev_request(
                &dev_url("submitDevelopmentCSR", device_type),
                body,
                "certRequest",
            )
            .await
            .context("Failed to submit development CSR")?;

        Ok(cert)
    }
}

impl CertificatesApi for DeveloperSession<'_> {
    fn developer_session(&self) -> &DeveloperSession<'_> {
        self
    }
}
