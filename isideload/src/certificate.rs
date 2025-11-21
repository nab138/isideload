// This file was made using https://github.com/Dadoum/Sideloader as a reference.

use hex;
use rcgen::{CertificateParams, DnType, KeyPair};
use rsa::pkcs1::EncodeRsaPublicKey;
use rsa::{
    RsaPrivateKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
};
use sha1::{Digest, Sha1};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use x509_certificate::X509Certificate;

use crate::Error;
use crate::developer_session::{DeveloperDeviceType, DeveloperSession, DeveloperTeam};

#[derive(Debug, Clone)]
pub struct CertificateIdentity {
    pub certificate: Option<X509Certificate>,
    pub private_key: RsaPrivateKey,
    pub key_file: PathBuf,
    pub cert_file: PathBuf,
    pub machine_name: String,
    pub machine_id: String,
}

impl CertificateIdentity {
    pub async fn new(
        configuration_path: &Path,
        dev_session: &DeveloperSession,
        apple_id: String,
        machine_name: String,
    ) -> Result<Self, Error> {
        let mut hasher = Sha1::new();
        hasher.update(apple_id.as_bytes());
        let hash_string = hex::encode(hasher.finalize()).to_lowercase();
        let key_path = configuration_path.join("keys").join(hash_string);
        fs::create_dir_all(&key_path).map_err(Error::Filesystem)?;

        let key_file = key_path.join("key.pem");
        let cert_file = key_path.join("cert.pem");
        let teams = dev_session.list_teams().await?;
        let team = teams
            .first()
            .ok_or(Error::Certificate("No teams found".to_string()))?;

        let private_key = if key_file.exists() {
            let key_data = fs::read_to_string(&key_file)
                .map_err(|e| Error::Certificate(format!("Failed to read key file: {}", e)))?;
            RsaPrivateKey::from_pkcs8_pem(&key_data)
                .map_err(|e| Error::Certificate(format!("Failed to load private key: {}", e)))?
        } else {
            let mut rng = rand::thread_rng();
            let private_key = RsaPrivateKey::new(&mut rng, 2048)
                .map_err(|e| Error::Certificate(format!("Failed to generate RSA key: {}", e)))?;

            let pem_data = private_key
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| Error::Certificate(format!("Failed to encode private key: {}", e)))?;
            fs::write(&key_file, pem_data.as_bytes()).map_err(Error::Filesystem)?;
            private_key
        };

        let mut cert_identity = CertificateIdentity {
            certificate: None,
            private_key,
            key_file,
            cert_file,
            machine_name,
            machine_id: "".to_owned(),
        };

        if let Ok((cert, machine_id)) = cert_identity
            .find_matching_certificate(dev_session, team)
            .await
        {
            cert_identity.certificate = Some(cert.clone());
            cert_identity.machine_id = machine_id;

            let cert_pem = cert
                .encode_pem()
                .map_err(|e| Error::Certificate(format!("Failed to encode cert: {}", e)))?;
            fs::write(&cert_identity.cert_file, cert_pem).map_err(Error::Filesystem)?;

            return Ok(cert_identity);
        }

        cert_identity
            .request_new_certificate(dev_session, team)
            .await?;
        Ok(cert_identity)
    }

    async fn find_matching_certificate(
        &self,
        dev_session: &DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<(X509Certificate, String), Error> {
        let certificates = dev_session
            .list_all_development_certs(DeveloperDeviceType::Ios, team)
            .await
            .map_err(|e| Error::Certificate(format!("Failed to list certificates: {:?}", e)))?;

        let our_public_key_der = self
            .private_key
            .to_public_key()
            .to_pkcs1_der()
            .map_err(|e| Error::Certificate(format!("Failed to get public key: {}", e)))?
            .to_vec();

        for cert in certificates
            .iter()
            .filter(|c| c.machine_name == self.machine_name)
        {
            if let Ok(x509_cert) = X509Certificate::from_der(&cert.cert_content) {
                let cert_public_key_der: Vec<u8> = x509_cert
                    .tbs_certificate()
                    .subject_public_key_info
                    .subject_public_key
                    .octets()
                    .collect();
                if cert_public_key_der == our_public_key_der {
                    return Ok((x509_cert, cert.machine_id.clone()));
                }
            }
        }
        Err(Error::Certificate(
            "No matching certificate found".to_string(),
        ))
    }

    async fn request_new_certificate(
        &mut self,
        dev_session: &DeveloperSession,
        team: &DeveloperTeam,
    ) -> Result<(), Error> {
        let mut params = CertificateParams::new(vec!["CN".to_string()])
            .map_err(|e| Error::Certificate(format!("Failed to create params: {}", e)))?;
        params.distinguished_name.push(DnType::CountryName, "US");
        params
            .distinguished_name
            .push(DnType::StateOrProvinceName, "STATE");
        params
            .distinguished_name
            .push(DnType::LocalityName, "LOCAL");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "ORGNIZATION");
        params.distinguished_name.push(DnType::CommonName, "CN");

        let key_pem = self
            .private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| Error::Certificate(format!("Failed to encode private key: {}", e)))?;
        let key_pair = KeyPair::from_pem(&key_pem)
            .map_err(|e| Error::Certificate(format!("Failed to load key pair for CSR: {}", e)))?;

        let csr = params
            .serialize_request(&key_pair)
            .map_err(|e| Error::Certificate(format!("Failed to generate CSR: {}", e)))?;
        let csr_pem = csr
            .pem()
            .map_err(|e| Error::Certificate(format!("Failed to encode CSR to PEM: {}", e)))?;

        let certificate_id = dev_session
            .submit_development_csr(
                DeveloperDeviceType::Ios,
                team,
                csr_pem,
                self.machine_name.clone(),
            )
            .await
            .map_err(|e| {
                let is_7460 = match &e {
                    Error::DeveloperSession(code, _) => *code == 7460,
                    _ => false,
                };
                if is_7460 {
                    Error::Certificate("You have too many certificates!".to_string())
                } else {
                    Error::Certificate(format!("Failed to submit CSR: {:?}", e))
                }
            })?;

        let certificates = dev_session
            .list_all_development_certs(DeveloperDeviceType::Ios, team)
            .await?;

        let apple_cert = certificates
            .iter()
            .find(|cert| cert.certificate_id == certificate_id)
            .ok_or(Error::Certificate(
                "Certificate not found after submission".to_string(),
            ))?;

        let certificate = X509Certificate::from_der(&apple_cert.cert_content)
            .map_err(|e| Error::Certificate(format!("Failed to parse certificate: {}", e)))?;

        // Write certificate to disk
        let cert_pem = certificate
            .encode_pem()
            .map_err(|e| Error::Certificate(format!("Failed to encode cert: {}", e)))?;
        fs::write(&self.cert_file, cert_pem).map_err(Error::Filesystem)?;

        self.certificate = Some(certificate);
        self.machine_id = apple_cert.machine_id.clone();

        Ok(())
    }

    pub fn get_certificate_file_path(&self) -> &Path {
        &self.cert_file
    }

    pub fn get_private_key_file_path(&self) -> &Path {
        &self.key_file
    }

    pub fn get_serial_number(&self) -> Result<String, Error> {
        let cert = match &self.certificate {
            Some(c) => c,
            None => {
                return Err(Error::Certificate(
                    "No certificate available to get serial number".to_string(),
                ));
            }
        };

        let serial = &cert.tbs_certificate().serial_number;
        let hex_str = hex::encode(serial.as_slice());

        Ok(hex_str.trim_start_matches("0").to_string())
    }

    pub fn to_pkcs12(&self, password: &str) -> Result<Vec<u8>, Error> {
        let output = Command::new("openssl")
            .arg("pkcs12")
            .arg("-export")
            .arg("-inkey")
            .arg(&self.key_file)
            .arg("-in")
            .arg(&self.cert_file)
            .arg("-passout")
            .arg(format!("pass:{}", password))
            .output()
            .map_err(|e| Error::Certificate(format!("Failed to execute openssl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Certificate(format!(
                "openssl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(output.stdout)
    }
}
