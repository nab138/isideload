use rcgen::{CertificateParams, DistinguishedName, DnType, PKCS_RSA_SHA256};
use rootcause::prelude::*;
use rsa::{
    RsaPrivateKey,
    pkcs8::{DecodePrivateKey, EncodePublicKey},
};
use sha2::{Digest, Sha256};
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
        apple_email: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
        storage: &dyn SideloadingStorage,
    ) -> Result<Self, Report> {
        let stored = Self::retrieve_from_storage(
            machine_name,
            apple_email,
            developer_session,
            team,
            storage,
        )
        .await;
        if let Ok(Some(cert)) = stored {
            return Ok(cert);
        }

        if let Err(e) = stored {
            error!("Failed to load stored certificate: {:?}", e);
        } else {
            info!("No stored certificate found");
        }

        todo!("generate CSR")
    }

    async fn retrieve_from_storage(
        machine_name: &str,
        apple_email: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
        storage: &dyn SideloadingStorage,
    ) -> Result<Option<Self>, Report> {
        let mut hasher = Sha256::new();
        hasher.update(apple_email.as_bytes());
        let email_hash = hex::encode(hasher.finalize());

        let private_key = storage.retrieve(&format!("{}/key", email_hash))?;
        if private_key.is_none() {
            return Ok(None);
        }
        let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key.unwrap())?;
        let public_key_der = private_key.to_public_key().to_public_key_der()?;

        for cert in developer_session
            .list_ios_certs(team)
            .await?
            .iter()
            .filter(|c| {
                c.cert_content.is_some() && c.machine_name.as_deref().unwrap_or("") == machine_name
            })
        {
            let x509_cert =
                x509_parser::parse_x509_certificate(cert.cert_content.as_ref().unwrap().as_ref())?;
            if x509_cert
                .1
                .tbs_certificate
                .subject_pki
                .subject_public_key
                .data
                == public_key_der.as_ref()
            {
                return Ok(Some(Self {
                    machine_id: cert
                        .machine_id
                        .clone()
                        .unwrap_or_else(|| x509_cert.1.tbs_certificate.subject.to_string()),
                    machine_name: machine_name.to_string(),
                }));
            }
        }

        Ok(None)
    }

    async fn request_certificate(
        machine_name: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<Self, Report> {
        Ok(())
    }

    fn build_csr(private_key: &RsaPrivateKey) -> Result<String, Report> {
        let mut params = CertificateParams::new(vec![])?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CountryName, "US");
        dn.push(DnType::StateOrProvinceName, "STATE");
        dn.push(DnType::LocalityName, "LOCAL");
        dn.push(DnType::OrganizationName, "ORGNIZATION");
        dn.push(DnType::CommonName, "CN");
        params.distinguished_name = dn;

        Ok(())
    }
}
