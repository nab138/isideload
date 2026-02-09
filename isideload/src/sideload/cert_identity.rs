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
    SideloadError,
    dev::{
        certificates::{CertificatesApi, DevelopmentCertificate},
        developer_session::DeveloperSession,
        teams::DeveloperTeam,
    },
    sideload::builder::MaxCertsBehavior,
    util::storage::SideloadingStorage,
};

pub struct CertificateIdentity {
    pub machine_id: String,
    pub machine_name: String,
    pub certificate: X509Certificate,
    pub private_key: RsaPrivateKey,
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
        if let Ok(Some((cert, x509_cert))) = found {
            info!("Found matching certificate");
            return Ok(Self {
                machine_id: cert.machine_id.clone().unwrap_or_default(),
                machine_name: cert.machine_name.clone().unwrap_or_default(),
                certificate: x509_cert,
                private_key: pr,
            });
        }

        if let Err(e) = found {
            error!("Failed to check for matching certificate: {:?}", e);
        }
        info!("Requesting new certificate");
        let (cert, x509_cert) = Self::request_certificate(
            &pr,
            machine_name.to_string(),
            developer_session,
            team,
            max_certs_behavior,
        )
        .await?;

        info!("Successfully obtained certificate");

        Ok(Self {
            machine_id: cert.machine_id.clone().unwrap_or_default(),
            machine_name: cert.machine_name.clone().unwrap_or_default(),
            certificate: x509_cert,
            private_key: pr,
        })
    }

    async fn retrieve_private_key(
        apple_email: &str,
        storage: &dyn SideloadingStorage,
    ) -> Result<RsaPrivateKey, Report> {
        let mut hasher = Sha256::new();
        hasher.update(apple_email.as_bytes());
        let email_hash = hex::encode(hasher.finalize());

        let private_key = storage.retrieve_data(&format!("{}/key", email_hash))?;
        if private_key.is_some() {
            return Ok(RsaPrivateKey::from_pkcs8_der(&private_key.unwrap())?);
        }

        let mut rng = rand::rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
        storage.store_data(
            &format!("{}/key", email_hash),
            &private_key.to_pkcs8_der()?.as_bytes(),
        )?;

        Ok(private_key)
    }

    async fn find_matching(
        private_key: &RsaPrivateKey,
        machine_name: &str,
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<Option<(DevelopmentCertificate, X509Certificate)>, Report> {
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
                return Ok(Some((cert.clone(), x509_cert)));
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
    ) -> Result<(DevelopmentCertificate, X509Certificate), Report> {
        let csr = Self::build_csr(private_key).context("Failed to generate CSR")?;

        let mut i = 0;
        let mut existing_certs: Option<Vec<DevelopmentCertificate>> = None;

        while i < 4 {
            i += 1;

            let result = developer_session
                .submit_development_csr(team, csr.clone(), machine_name.clone(), None)
                .await;

            match result {
                Ok(request) => {
                    let apple_certs = developer_session.list_ios_certs(team).await?;

                    let apple_cert = apple_certs
                        .iter()
                        .find(|c| c.certificate_id == Some(request.cert_request_id.clone()))
                        .ok_or_else(|| {
                            report!("Failed to find certificate after submitting CSR")
                        })?;

                    let x509_cert = X509Certificate::from_der(
                        apple_cert
                            .cert_content
                            .as_ref()
                            .ok_or_else(|| report!("Certificate content missing"))?
                            .as_ref(),
                    )?;

                    return Ok((apple_cert.clone(), x509_cert));
                }
                Err(e) => {
                    let error = e
                        .iter_reports()
                        .find_map(|node| node.downcast_current_context::<SideloadError>());
                    if let Some(SideloadError::DeveloperError(code, _)) = error {
                        if *code == 7460 {
                            if existing_certs.is_none() {
                                existing_certs = Some(
                                    developer_session
                                        .list_ios_certs(team)
                                        .await?
                                        .iter()
                                        .filter(|c| c.serial_number.is_some())
                                        .cloned()
                                        .collect(),
                                );
                            }
                            Self::revoke_others(
                                developer_session,
                                team,
                                max_certs_behavior,
                                SideloadError::DeveloperError(
                                    *code,
                                    "Maximum number of certificates reached".to_string(),
                                ),
                                &mut existing_certs.as_mut().unwrap(),
                            )
                            .await?;
                        } else {
                            return Err(e);
                        }
                    }
                }
            };
        }

        Err(report!("Reached max attempts to request certificate"))
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

    async fn revoke_others(
        developer_session: &mut DeveloperSession,
        team: &DeveloperTeam,
        max_certs_behavior: &MaxCertsBehavior,
        error: SideloadError,
        existing_certs: &mut Vec<DevelopmentCertificate>,
    ) -> Result<(), Report> {
        match max_certs_behavior {
            MaxCertsBehavior::Revoke => {
                if let Some(cert) = existing_certs.pop() {
                    info!(
                        "Revoking certificate with name: {:?} ({:?})",
                        cert.name, cert.machine_name
                    );
                    developer_session
                        .revoke_development_cert(team, &cert.serial_number.unwrap(), None)
                        .await?;
                    Ok(())
                } else {
                    error!("No more certificates to revoke but still hitting max certs error");
                    return Err(error.into());
                }
            }
            MaxCertsBehavior::Error => Err(error.into()),
            MaxCertsBehavior::Prompt(prompt_fn) => {
                let certs_to_revoke = prompt_fn(existing_certs);
                if certs_to_revoke.is_none() {
                    error!("User did not select any certificates to revoke");
                    return Err(error.into());
                }
                for cert in certs_to_revoke.unwrap() {
                    info!(
                        "Revoking certificate with name: {}",
                        cert.machine_name
                            .unwrap_or(cert.machine_id.unwrap_or_default())
                    );
                    let serial_number = cert.serial_number.clone();
                    developer_session
                        .revoke_development_cert(team, &cert.serial_number.unwrap(), None)
                        .await?;
                    existing_certs.retain(|c| c.serial_number != serial_number);
                }
                Ok(())
            }
        }
    }
}
