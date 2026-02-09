use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_RSA_SHA256};
use rootcause::prelude::*;
use rsa::{
    RsaPrivateKey,
    pkcs1::EncodeRsaPublicKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
};

use sha2::{Digest, Sha256};
use tracing::{error, info};
use x509_certificate::X509Certificate;

use crate::{
    dev::{
        certificates::CertificatesApi, developer_session::DeveloperSession, teams::DeveloperTeam,
    },
    sideload::builder::MaxCertsBehavior,
    util::storage::SideloadingStorage,
};

pub struct CertificateIdentity {
    pub machine_id: String,
    pub machine_name: String,
    pub certificate: X509Certificate,
}

impl CertificateIdentity {
    pub async fn retrieve(
        machine_name: &str,
        apple_email: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
        storage: &dyn SideloadingStorage,
        max_certs_behavior: &MaxCertsBehavior,
    ) -> Result<Self, Report> {
        let pr = Self::retrieve_private_key(apple_email, storage).await?;

        let found = Self::find_matching(&pr, machine_name, developer_session, team).await;
        if let Ok(Some(cert)) = found {
            info!("Found matching certificate");
            return Ok(cert);
        }

        if let Err(e) = found {
            error!("Failed to check for matching certificate: {:?}", e);
        }
        info!("Requesting new certificate");
        Self::request_certificate(
            &pr,
            machine_name.to_string(),
            developer_session,
            team,
            max_certs_behavior,
        )
        .await
    }

    async fn retrieve_private_key(
        apple_email: &str,
        storage: &dyn SideloadingStorage,
    ) -> Result<RsaPrivateKey, Report> {
        let mut hasher = Sha256::new();
        hasher.update(apple_email.as_bytes());
        let email_hash = hex::encode(hasher.finalize());

        let private_key = storage.retrieve(&format!("{}/key", email_hash))?;
        if private_key.is_some() {
            return Ok(RsaPrivateKey::from_pkcs8_pem(&private_key.unwrap())?);
        }

        let mut rng = rand::rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
        storage.store(
            &format!("{}/key", email_hash),
            &private_key.to_pkcs8_pem(Default::default())?.to_string(),
        )?;

        Ok(private_key)
    }

    async fn find_matching(
        private_key: &RsaPrivateKey,
        machine_name: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<Option<Self>, Report> {
        let public_key_der = private_key
            .to_public_key()
            .to_pkcs1_der()?
            .as_bytes()
            .to_vec();
        for cert in developer_session
            .list_ios_certs(team)
            .await?
            .iter()
            .filter(|c| {
                c.cert_content.is_some()
                    && c.machine_name.as_deref().unwrap_or("") == machine_name
                    && c.machine_id.is_some()
            })
        {
            let x509_cert =
                X509Certificate::from_der(cert.cert_content.as_ref().unwrap().as_ref())?;

            if public_key_der == x509_cert.public_key_data().as_ref() {
                return Ok(Some(Self {
                    machine_id: cert.machine_id.clone().unwrap_or_default(),
                    machine_name: cert.machine_name.clone().unwrap_or_default(),
                    certificate: x509_cert,
                }));
            }
        }

        Ok(None)
    }

    async fn request_certificate(
        private_key: &RsaPrivateKey,
        machine_name: String,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
        max_certs_behavior: &MaxCertsBehavior,
    ) -> Result<Self, Report> {
        let csr = Self::build_csr(private_key).context("Failed to generate CSR")?;

        let request = developer_session
            .submit_development_csr(team, csr, machine_name, None)
            .await?;

        // TODO: Handle max certs behavior properly instead of just always revoking

        let apple_certs = developer_session.list_ios_certs(team).await?;

        let apple_cert = apple_certs
            .iter()
            .find(|c| c.certificate_id == Some(request.cert_request_id.clone()))
            .ok_or_else(|| report!("Failed to find certificate after submitting CSR"))?;

        let x509_cert = X509Certificate::from_der(
            apple_cert
                .cert_content
                .as_ref()
                .ok_or_else(|| report!("Certificate content missing"))?
                .as_ref(),
        )?;

        Ok(Self {
            machine_id: apple_cert.machine_id.clone().unwrap_or_default(),
            machine_name: apple_cert.machine_name.clone().unwrap_or_default(),
            certificate: x509_cert,
        })
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

        let subject_key = KeyPair::from_pkcs8_pem_and_sign_algo(
            &private_key.to_pkcs8_pem(LineEnding::LF)?,
            &PKCS_RSA_SHA256,
        )?;

        Ok(params.serialize_request(&subject_key)?.pem()?)
    }
}
