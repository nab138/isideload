use crate::{
    SideloadResult as Result,
    anisette::{AnisetteProvider, remote_v3::RemoteV3AnisetteProvider},
    auth::grandslam::GrandSlam,
};
use log::{info, warn};
use reqwest::{Certificate, ClientBuilder};
use thiserror_context::Context;

const APPLE_ROOT: &[u8] = include_bytes!("./apple_root.der");

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub client: reqwest::Client,
    pub anisette: Box<dyn AnisetteProvider>,
}

pub struct AppleAccountBuilder {
    email: String,
    debug: Option<bool>,
    anisette: Option<Box<dyn AnisetteProvider>>,
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
            anisette: None,
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

    pub fn anisette(mut self, anisette: impl AnisetteProvider + 'static) -> Self {
        self.anisette = Some(Box::new(anisette));
        self
    }

    /// Build the AppleAccount and log in
    ///
    /// # Arguments
    /// - `password`: The Apple ID password
    /// - `two_factor_callback`: A callback function that returns the two-factor authentication code
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub async fn login<F>(self, _password: &str, _two_factor_callback: F) -> Result<AppleAccount>
    where
        F: Fn() -> Result<String>,
    {
        let debug = self.debug.unwrap_or(false);
        let anisette = self
            .anisette
            .unwrap_or_else(|| Box::new(RemoteV3AnisetteProvider::default()));

        AppleAccount::login(&self.email, debug, anisette).await
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
    pub async fn login(
        email: &str,
        debug: bool,
        anisette: Box<dyn AnisetteProvider>,
    ) -> Result<Self> {
        info!("Logging in to apple ID: {}", email);
        if debug {
            warn!("Debug mode enabled: this is a security risk!");
        }
        let client = Self::build_client(debug)?;

        let mut gs = GrandSlam::new(&client);
        gs.get_url_bag()
            .await
            .context("Failed to get URL bag from GrandSlam")?;

        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            client,
            anisette,
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
