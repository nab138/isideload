use crate::Result;
use reqwest::{Certificate, ClientBuilder};

const APPLE_ROOT: &[u8] = include_bytes!("./apple_root.der");

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub client: reqwest::Client,
    pub anisette: Box<dyn crate::anisette::AnisetteProvider>,
}

impl AppleAccount {
    /// Create a new AppleAccount with the given email
    ///
    /// # Arguments
    /// - `email`: The Apple ID email address
    pub fn new(email: &str) -> Result<Self> {
        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            client: Self::build_client(false)?,
            anisette: Box::new(crate::anisette::DefaultAnisetteProvider {}),
        })
    }

    /// Build a reqwest client with the Apple root certificate
    ///
    /// # Arguments
    /// - `debug`: DANGER, If true, accept invalid certificates and enable verbose connection logging
    pub fn build_client(debug: bool) -> Result<reqwest::Client> {
        let cert = Certificate::from_der(APPLE_ROOT)?;
        let client = ClientBuilder::new()
            .add_root_certificate(cert)
            .http1_title_case_headers()
            .danger_accept_invalid_certs(debug)
            .connection_verbose(debug)
            .build()?;

        Ok(client)
    }
}
