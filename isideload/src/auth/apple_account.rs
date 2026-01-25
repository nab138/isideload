use crate::{
    anisette::{AnisetteProvider, remote_v3::RemoteV3AnisetteProvider},
    auth::grandslam::GrandSlam,
};
use rootcause::prelude::*;
use tracing::{info, warn};

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub anisette: Box<dyn AnisetteProvider>,
    pub grandslam_client: GrandSlam,
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
    pub async fn login<F>(
        self,
        password: &str,
        two_factor_callback: F,
    ) -> Result<AppleAccount, Report>
    where
        F: Fn() -> Option<String>,
    {
        let debug = self.debug.unwrap_or(false);
        let anisette = self
            .anisette
            .unwrap_or_else(|| Box::new(RemoteV3AnisetteProvider::default()));

        AppleAccount::login(&self.email, password, two_factor_callback, anisette, debug).await
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
        password: &str,
        two_factor_callback: impl Fn() -> Option<String>,
        mut anisette: Box<dyn AnisetteProvider>,
        debug: bool,
    ) -> Result<Self, Report> {
        info!("Logging in to apple ID: {}", email);
        if debug {
            warn!("Debug mode enabled: this is a security risk!");
        }

        let client_info = anisette
            .get_client_info()
            .await
            .context("Failed to get anisette client info")?;
        let mut grandslam_client = GrandSlam::new(client_info, debug);
        let url_bag = grandslam_client
            .get_url_bag()
            .await
            .context("Failed to get URL bag for login")?;

        let headers = anisette
            .get_anisette_headers(&mut grandslam_client)
            .await
            .context("Failed to get anisette headers for login")?;

        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            anisette,
            grandslam_client,
        })
    }
}
