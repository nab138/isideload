use crate::SideloadResult as Result;
use log::debug;
use plist::Dictionary;
use plist_macro::pretty_print_dictionary;
use reqwest::header::HeaderValue;

const URL_BAG: &str = "https://gsa.apple.com/grandslam/GsService2/lookup";

pub struct GrandSlam<'a> {
    client: &'a reqwest::Client,
    url_bag: Option<Dictionary>,
}

impl<'a> GrandSlam<'a> {
    /// Create a new GrandSlam instance
    ///
    /// # Arguments
    /// - `client`: The reqwest client to use for requests
    pub fn new(client: &'a reqwest::Client) -> Self {
        Self {
            client,
            url_bag: None,
        }
    }

    /// Get the URL bag from GrandSlam
    pub async fn get_url_bag(&mut self) -> Result<&Dictionary> {
        if self.url_bag.is_none() {
            debug!("Fetching URL bag from GrandSlam");
            let resp = self
                .client
                .get(URL_BAG)
                .headers(Self::base_headers())
                .send()
                .await?
                .text()
                .await?;
            let dict: Dictionary = plist::from_bytes(resp.as_bytes())?;
            debug!("{}", pretty_print_dictionary(&dict));
            self.url_bag = Some(dict);
        }
        Ok(self.url_bag.as_ref().unwrap())
    }

    fn base_headers() -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Context-Type", HeaderValue::from_static("text/x-xml-plist"));
        headers.insert("Accept", HeaderValue::from_static("text/x-xml-plist"));
        headers.insert(
            "X-Mme-Client-Info",
            HeaderValue::from_static(
                "<MacBookPro13,2> <macOS;13.1;22C65> <com.apple.AuthKit/1 (com.apple.dt.Xcode/3594.4.19)>",
            ),
        );
        headers.insert("User-Agent", HeaderValue::from_static("Xcode"));
        headers.insert("X-Xcode-Version", HeaderValue::from_static("14.2 (14C18)"));
        headers.insert(
            "X-Apple-App-Info",
            HeaderValue::from_static("com.apple.gs.xcode.auth"),
        );

        headers
    }
}
