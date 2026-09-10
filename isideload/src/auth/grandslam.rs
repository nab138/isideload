use super::middleware::WasmProxyMiddleware;
use plist::Dictionary;
use plist_macro::plist_to_xml_string;
use plist_macro::pretty_print_dictionary;
#[cfg(not(feature = "wasm"))]
use reqwest::Certificate;
use reqwest::{
    ClientBuilder,
    header::{HeaderMap, HeaderValue},
};
use reqwest_middleware::ClientBuilder as MwClientBuilder;
use rootcause::prelude::*;
use std::time::Duration;
use tracing::{debug, warn};

#[cfg(not(feature = "wasm"))]
use crate::sideload::cert_identity::APPLE_ROOT;
use crate::{SideloadError, anisette::AnisetteClientInfo, util::plist::PlistDataExtract};

const URL_BAG: &str = "https://gsa.apple.com/grandslam/GsService2/lookup";

/// How many times a GrandSlam plist request is attempted before giving up.
const MAX_GSA_ATTEMPTS: u32 = 5;
/// Base delay for the exponential backoff between retries (0.5s, 1s, 2s, 4s).
const GSA_RETRY_BASE_DELAY_MS: u64 = 500;

pub struct GrandSlam {
    pub client: reqwest_middleware::ClientWithMiddleware,
    pub client_info: AnisetteClientInfo,
    url_bag: Dictionary,
}

impl GrandSlam {
    /// Create a new GrandSlam instance
    ///
    /// # Arguments
    /// - `client`: The reqwest client to use for requests
    pub async fn new(
        client_info: AnisetteClientInfo,
        debug: bool,
        proxy_url: Option<String>,
    ) -> Result<Self, Report> {
        let client =
            Self::build_reqwest_client(debug, proxy_url).context("Failed to build HTTP client")?;
        let base_headers = Self::base_headers(&client_info, false)?;
        let url_bag = Self::fetch_url_bag(&client, base_headers).await?;
        Ok(Self {
            client,
            client_info,
            url_bag,
        })
    }

    /// Fetch the URL bag from GrandSlam and cache it
    pub async fn fetch_url_bag(
        client: &reqwest_middleware::ClientWithMiddleware,
        base_headers: HeaderMap,
    ) -> Result<Dictionary, Report> {
        debug!("Fetching URL bag from GrandSlam");
        let resp = client
            .get(URL_BAG)
            .headers(base_headers)
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

        Ok(urls)
    }

    pub fn get_url(&self, key: &str) -> Result<String, Report> {
        let url = self
            .url_bag
            .get_string(key)
            .context("Unable to find key in URL bag")?;
        Ok(url)
    }

    pub fn get(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .get(url)
            .headers(Self::base_headers(&self.client_info, false)?);

        Ok(builder)
    }

    pub fn get_sms(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .get(url)
            .headers(Self::base_headers(&self.client_info, true)?);

        Ok(builder)
    }

    pub fn put_sms(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .put(url)
            .headers(Self::base_headers(&self.client_info, true)?);

        Ok(builder)
    }

    pub fn post(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .post(url)
            .headers(Self::base_headers(&self.client_info, false)?);

        Ok(builder)
    }

    pub fn post_sms(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .post(url)
            .headers(Self::base_headers(&self.client_info, true)?);

        Ok(builder)
    }

    pub fn patch(&self, url: &str) -> Result<reqwest_middleware::RequestBuilder, Report> {
        let builder = self
            .client
            .patch(url)
            .headers(Self::base_headers(&self.client_info, false)?);

        Ok(builder)
    }

    /// Send a plist request to GrandSlam.
    ///
    /// Server errors are retried with exponential backoff, and the response is checked for a
    /// plausible plist body before it is handed to the parser, so an HTML error page reports its
    /// status code instead of an opaque "unknown tag html on line 1".
    pub async fn plist_request(
        &self,
        url: &str,
        body: &Dictionary,
        additional_headers: Option<HeaderMap>,
    ) -> Result<Dictionary, Report> {
        let body_xml = plist_to_xml_string(body);
        let extra_headers = additional_headers.unwrap_or_else(reqwest::header::HeaderMap::new);

        let mut attempt: u32 = 0;

        let resp = loop {
            attempt += 1;

            let response = self
                .client
                .post(url)
                .headers(Self::base_headers(&self.client_info, false)?)
                .headers(extra_headers.clone())
                .body(body_xml.clone())
                .send()
                .await
                .context("Failed to send grandslam request")?;

            let status = response.status();

            if status.is_server_error() && attempt < MAX_GSA_ATTEMPTS {
                let delay = Duration::from_millis(GSA_RETRY_BASE_DELAY_MS << (attempt - 1));
                warn!(
                    "GrandSlam returned HTTP {} (attempt {}/{}), retrying in {:?}",
                    status.as_u16(),
                    attempt,
                    MAX_GSA_ATTEMPTS,
                    delay
                );
                Self::backoff(delay).await;
                continue;
            }

            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<none>")
                .to_string();

            let text = response
                .text()
                .await
                .context("Failed to read grandslam response as text")?;

            if !status.is_success() || !Self::looks_like_plist(&content_type, &text) {
                return Err(report!(
                    "GrandSlam returned an unexpected response: HTTP {} (Content-Type: {}) after {} attempt(s)",
                    status.as_u16(),
                    content_type,
                    attempt
                )
                .attach(Self::body_snippet(&text)));
            }

            break text;
        };

        let dict: Dictionary = plist::from_bytes(resp.as_bytes())
            .context("Failed to parse grandslam response plist")
            .attach_with(|| resp.clone())?;

        let response_plist = dict
            .get("Response")
            .and_then(|v| v.as_dictionary())
            .cloned()
            .ok_or_else(|| {
                report!("grandslam response missing 'Response'")
                    .attach(pretty_print_dictionary(&dict))
            })?;

        Ok(response_plist)
    }

    /// Sleep between retries. `tokio::time` is unavailable on wasm, where the retry simply
    /// happens immediately on a new connection.
    #[allow(unused_variables)]
    async fn backoff(delay: Duration) {
        #[cfg(not(feature = "wasm"))]
        tokio::time::sleep(delay).await;
    }

    /// Cheap sanity check that the body could be a plist at all. Apple serves HTML error pages
    /// from the same endpoint, and feeding those to the plist parser hides the real failure.
    fn looks_like_plist(content_type: &str, body: &str) -> bool {
        let content_type = content_type.to_ascii_lowercase();
        if content_type.contains("html") {
            return false;
        }

        let trimmed = body.trim_start();
        let head = trimmed
            .chars()
            .take(64)
            .collect::<String>()
            .to_ascii_lowercase();
        !(head.starts_with("<!doctype html") || head.starts_with("<html"))
    }

    /// First 512 characters of a response body, flattened onto one line, for error reports.
    fn body_snippet(body: &str) -> String {
        let flattened = body.split_whitespace().collect::<Vec<_>>().join(" ");
        let snippet = flattened.chars().take(512).collect::<String>();
        if flattened.chars().count() > 512 {
            format!("Body: {snippet}...")
        } else {
            format!("Body: {snippet}")
        }
    }

    fn base_headers(
        client_info: &AnisetteClientInfo,
        sms: bool,
    ) -> Result<reqwest::header::HeaderMap, Report> {
        let mut headers = reqwest::header::HeaderMap::new();
        if !sms {
            headers.insert("Content-Type", HeaderValue::from_static("text/x-xml-plist"));
            headers.insert("Accept", HeaderValue::from_static("text/x-xml-plist"));
        } else {
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));
            headers.insert("Accept", HeaderValue::from_static("application/json"));
        }
        headers.insert(
            "X-Mme-Client-Info",
            HeaderValue::from_str(&client_info.client_info)?,
        );
        headers.insert(
            "User-Agent",
            HeaderValue::from_str(&client_info.user_agent)?,
        );
        headers.insert(
            "X-Xcode-Version",
            HeaderValue::from_static("27.0 (27A5218g)"),
        );
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
    pub fn build_reqwest_client(
        debug: bool,
        proxy_url: Option<String>,
    ) -> Result<reqwest_middleware::ClientWithMiddleware, Report> {
        #[cfg(not(feature = "wasm"))]
        let cert = Certificate::from_der(APPLE_ROOT)?;
        #[cfg(not(feature = "wasm"))]
        let client = ClientBuilder::new()
            .add_root_certificate(cert)
            .http1_title_case_headers()
            .danger_accept_invalid_certs(debug)
            .connection_verbose(debug)
            .build()?;
        #[cfg(feature = "wasm")]
        let client = ClientBuilder::new().build()?;

        let builder = MwClientBuilder::new(client);
        let builder = if let Some(proxy_url) = proxy_url {
            builder.with(WasmProxyMiddleware::new(proxy_url))
        } else {
            builder
        };
        Ok(builder.build())
    }
}

pub trait GrandSlamErrorChecker {
    fn check_grandslam_error(self) -> Result<Dictionary, Report<SideloadError>>;
}

impl GrandSlamErrorChecker for Dictionary {
    fn check_grandslam_error(self) -> Result<Self, Report<SideloadError>> {
        let result = match self.get("Status") {
            Some(plist::Value::Dictionary(d)) => d,
            _ => &self,
        };

        if result.get_signed_integer("ec").unwrap_or(0) != 0 {
            bail!(SideloadError::AuthWithMessage(
                result.get_signed_integer("ec").unwrap_or(-1),
                result.get_str("em").unwrap_or("Unknown error").to_string(),
            ))
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const HTML_503: &str = "<html><head><title>503 Service Temporarily Unavailable</title></head>\
         <body>Service Temporarily Unavailable</body></html>";
    const VALID_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>Response</key><dict><key>ec</key><integer>0</integer></dict></dict></plist>"#;

    fn test_grandslam() -> GrandSlam {
        // reqwest is built with `rustls-no-provider`, so a provider has to be installed before
        // any Client can be constructed. Binaries do this in main(); tests have to do it here.
        static CRYPTO_PROVIDER: std::sync::Once = std::sync::Once::new();
        CRYPTO_PROVIDER.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });

        GrandSlam {
            client: GrandSlam::build_reqwest_client(false, None).unwrap(),
            client_info: AnisetteClientInfo {
                client_info: "<test>".to_string(),
                user_agent: "test".to_string(),
            },
            url_bag: Dictionary::new(),
        }
    }

    /// Serves one canned response per connection and counts the requests it received. The
    /// response asks for the connection to be closed, so one accepted connection is exactly one
    /// request whether or not the client pools connections.
    async fn spawn_server(
        status_line: &'static str,
        content_type: &'static str,
        body: &'static str,
    ) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = requests.clone();

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, Ordering::SeqCst);

                let mut buf = [0u8; 8192];
                let _ = socket.read(&mut buf).await;

                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });

        (format!("http://{addr}/GsService2"), requests)
    }

    #[tokio::test]
    async fn retries_an_html_503_up_to_the_attempt_limit() {
        let (url, requests) =
            spawn_server("503 Service Temporarily Unavailable", "text/html", HTML_503).await;

        let error = test_grandslam()
            .plist_request(&url, &Dictionary::new(), None)
            .await
            .expect_err("an HTML 503 must not be reported as a successful response");
        let rendered = format!("{error:?}");

        assert!(
            rendered.contains("503"),
            "error should name the status code, got: {rendered}"
        );
        assert!(
            rendered.contains("Service Temporarily Unavailable"),
            "error should carry a snippet of the HTML body, got: {rendered}"
        );
        assert!(
            !rendered.contains("unknown tag"),
            "error should not be a raw plist parse failure, got: {rendered}"
        );
        assert_eq!(
            requests.load(Ordering::SeqCst),
            MAX_GSA_ATTEMPTS as usize,
            "a 5xx must be retried up to the attempt limit"
        );
    }

    #[tokio::test]
    async fn parses_a_plist_response() {
        let (url, requests) = spawn_server("200 OK", "text/x-xml-plist", VALID_PLIST).await;

        let response = test_grandslam()
            .plist_request(&url, &Dictionary::new(), None)
            .await
            .expect("a valid plist should parse");

        assert_eq!(response.get_signed_integer("ec").unwrap(), 0);
        assert_eq!(requests.load(Ordering::SeqCst), 1, "no retry expected");
    }

    #[tokio::test]
    async fn rejects_html_served_with_a_200() {
        let (url, requests) = spawn_server("200 OK", "text/html", HTML_503).await;

        let error = test_grandslam()
            .plist_request(&url, &Dictionary::new(), None)
            .await
            .expect_err("HTML must be rejected even when the status is 200");

        assert!(format!("{error:?}").contains("text/html"));
        assert_eq!(
            requests.load(Ordering::SeqCst),
            1,
            "a 2xx must not be retried"
        );
    }
}
