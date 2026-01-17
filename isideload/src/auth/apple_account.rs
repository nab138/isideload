use crate::Result;
use reqwest::{Certificate, ClientBuilder};

const APPLE_ROOT: &[u8] = include_bytes!("./apple_root.der");

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub client: reqwest::Client,
    pub anisette: Box<dyn crate::anisette::AnisetteProvider>,
}

#[derive(Debug)]
pub struct AppleAccountBuilder {
    email: String,
    debug: Option<bool>,
}

impl AppleAccountBuilder {
    /// Create a new AppleAccountBuilder with the given email
    ///
    /// # Arguments
    /// - `email`: The Apple ID email address
    pub fn new(email: &str) -> Self {
        Self {
            email: email.to_string(),
            debug: None,
        }
    }

    /// DANGER Set whether to enable debug mode
    ///
    /// # Arguments
    /// - `debug`: If true, accept invalid certificates and enable verbose connection logging
    pub fn danger_debug(mut self, debug: bool) -> Self {
        self.debug = Some(debug);
        self
    }

    /// Build the AppleAccount
    ///
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub fn login(self) -> Result<AppleAccount> {
        let debug = self.debug.unwrap_or(false);

        AppleAccount::login(&self.email, debug)
    }
}

impl AppleAccount {
    /// Create a new AppleAccountBuilder with the given email
    ///
    /// # Arguments
    /// - `email`: The Apple ID email address
    pub fn builder(email: &str) -> AppleAccountBuilder {
        AppleAccountBuilder::new(email)
    }

    /// Log in to an Apple account with the given email
    ///
    /// Reccomended to use the AppleAccountBuilder instead
    pub fn login(email: &str, debug: bool) -> Result<Self> {
        let client = Self::build_client(debug)?;

        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            client,
            anisette: Box::new(crate::anisette::DefaultAnisetteProvider {}),
        })
    }

    /// Build a reqwest client with the Apple root certificate
    ///
    /// # Arguments
    /// - `debug`: DANGER, If true, accept invalid certificates and enable verbose connection logging
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
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
