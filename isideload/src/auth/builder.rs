use std::sync::Arc;

use rootcause::prelude::*;
use tokio::sync::RwLock;

use crate::util::callbacks::TwoFactorCallback;
use crate::{
    anisette::{AnisetteDataGenerator, AnisetteProvider, remote_v3::RemoteV3AnisetteProvider},
    auth::apple_account::AppleAccount,
};

pub struct AppleAccountBuilder {
    email: String,
    debug: Option<bool>,
    anisette_generator: Option<AnisetteDataGenerator>,
    proxy_url: Option<String>,
    err_429_retries: Option<u32>,
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
            anisette_generator: None,
            proxy_url: None,
            err_429_retries: Some(10),
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

    /// Set a proxy URL to use for all network requests, useful for running in a browser with WASM
    ///
    /// # Arguments
    /// - `proxy_url`: The proxy URL to use for all network requests
    pub fn proxy_url(mut self, proxy_url: Option<String>) -> Self {
        self.proxy_url = proxy_url;
        self
    }

    /// Set an anisette provider to use. RemoteV3AnisetteProvider is used by default if none is provided.
    /// You should create your own instance of RemoteV3AnisetteProvider if you want to use a different anisette server.
    ///
    /// # Arguments
    /// - `anisette_provider`: The anisette provider to use
    pub fn anisette_provider(
        mut self,
        anisette_provider: impl AnisetteProvider + Send + Sync + 'static,
    ) -> Self {
        self.anisette_generator = Some(AnisetteDataGenerator::new(Arc::new(RwLock::new(
            anisette_provider,
        ))));
        self
    }

    /// Sets the number of retries to attempt when receiving a 429 Too Many Requests response from Apple servers, default is 10.
    /// Set to None to disable retries.
    ///
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub fn err_429_retries(mut self, retries: Option<u32>) -> Self {
        self.err_429_retries = retries;
        self
    }

    /// Build the AppleAccount without logging in
    ///
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub async fn build(self) -> Result<AppleAccount, Report> {
        let debug = self.debug.unwrap_or(false);
        let anisette_generator = match self.anisette_generator {
            Some(generator) => generator,
            None => {
                let provider = RemoteV3AnisetteProvider::default()?;
                AnisetteDataGenerator::new(Arc::new(RwLock::new(provider)))
            }
        };

        AppleAccount::new(
            &self.email,
            anisette_generator,
            debug,
            self.proxy_url,
            self.err_429_retries,
        )
        .await
    }

    /// Build the AppleAccount and log in
    ///
    /// # Arguments
    /// - `password`: The Apple ID password
    /// - `two_factor_callback`: A callback function that returns the two-factor authentication code
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub async fn login<C>(
        self,
        password: &str,
        two_factor_callback: C,
    ) -> Result<AppleAccount, Report>
    where
        C: TwoFactorCallback,
    {
        let mut account = self.build().await?;
        account.login(password, two_factor_callback).await?;
        Ok(account)
    }
}
