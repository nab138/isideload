use plist::Dictionary;
use reqwest::{Certificate, ClientBuilder, header::HeaderValue};
use rootcause::prelude::*;
use tracing::debug;

use crate::anisette::AnisetteClientInfo;

const APPLE_ROOT: &[u8] = include_bytes!("./apple_root.der");
const URL_BAG: &str = "https://gsa.apple.com/grandslam/GsService2/lookup";

pub struct GrandSlam {
    pub client: reqwest::Client,
    pub client_info: AnisetteClientInfo,
    pub url_bag: Option<Dictionary>,
}

impl GrandSlam {
    /// Create a new GrandSlam instance
    ///
    /// # Arguments
    /// - `client`: The reqwest client to use for requests
    pub fn new(client_info: AnisetteClientInfo, debug: bool) -> Self {
        Self {
            client: Self::build_reqwest_client(debug).unwrap(),
            client_info,
            url_bag: None,
        }
    }

    /// Get the URL bag from GrandSlam
    pub async fn get_url_bag(&mut self) -> Result<&Dictionary, Report> {
        if self.url_bag.is_none() {
            debug!("Fetching URL bag from GrandSlam");
            let resp = self
                .client
                .get(URL_BAG)
                .headers(self.base_headers()?)
                .send()
                .await
                .context("Failed to fetch URL Bag")?
                .text()
                .await
                .context("Failed to read URL Bag response text")?;

            let dict: Dictionary =
                plist::from_bytes(resp.as_bytes()).context("Failed to parse URL Bag plist")?;
            let urls = dict
                .get("urls")
                .and_then(|v| v.as_dictionary())
                .cloned()
                .ok_or_else(|| report!("URL Bag plist missing 'urls' dictionary"))?;

            self.url_bag = Some(urls);
        }
        Ok(self.url_bag.as_ref().unwrap())
    }

    pub fn post(&self, url: &str) -> Result<reqwest::RequestBuilder, Report> {
        let builder = self.client.post(url).headers(self.base_headers()?);

        Ok(builder)
    }

    fn base_headers(&self) -> Result<reqwest::header::HeaderMap, Report> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("text/x-xml-plist"));
        headers.insert("Accept", HeaderValue::from_static("text/x-xml-plist"));
        headers.insert(
            "X-Mme-Client-Info",
            HeaderValue::from_str(&self.client_info.client_info)?,
        );
        headers.insert(
            "User-Agent",
            HeaderValue::from_str(&self.client_info.user_agent)?,
        );
        headers.insert("X-Xcode-Version", HeaderValue::from_static("14.2 (14C18)"));
        headers.insert(
            "X-Apple-App-Info",
            HeaderValue::from_static("com.apple.gs.xcode.auth"),
        );

        Ok(headers)
    }

    /// Build a reqwest client with the Apple root certificate
    ///
    /// # Arguments
    /// - `debug`: DANGER, If true, accept invalid certificates and enable verbose connection logging
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub fn build_reqwest_client(debug: bool) -> Result<reqwest::Client, Report> {
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
