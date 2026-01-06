use reqwest::{Certificate, ClientBuilder};

const APPLE_ROOT: &[u8] = include_bytes!("./apple_root.der");

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub client: reqwest::Client,
}

impl AppleAccount {
    pub fn new(email: &str) -> reqwest::Result<Self> {
        let client = ClientBuilder::new()
            .add_root_certificate(Certificate::from_der(APPLE_ROOT)?)
            // uncomment when debugging w/ charles proxy
            // .danger_accept_invalid_certs(true)
            .http1_title_case_headers()
            .connection_verbose(true)
            .build()?;

        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            client,
        })
    }
}
