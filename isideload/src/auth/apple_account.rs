use crate::{
    anisette::{AnisetteData, AnisetteProvider, remote_v3::RemoteV3AnisetteProvider},
    auth::grandslam::{GrandSlam, GrandSlamErrorChecker},
    util::plist::PlistDataExtract,
};
use aes::cipher::block_padding::Pkcs7;
use base64::{Engine, prelude::BASE64_STANDARD};
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use plist::Dictionary;
use plist_macro::plist;
use reqwest::header::{HeaderMap, HeaderValue};
use rootcause::prelude::*;
use sha2::{Digest, Sha256};
use srp::{
    client::{SrpClient, SrpClientVerifier},
    groups::G_2048,
};
use tracing::{debug, info, warn};

const SERIAL_NUMBER: &str = "2";

pub struct AppleAccount {
    pub email: String,
    pub spd: Option<plist::Dictionary>,
    pub anisette_provider: Box<dyn AnisetteProvider>,
    pub anisette_data: AnisetteData,
    pub grandslam_client: GrandSlam,
    login_state: LoginState,
    debug: bool,
}

pub struct AppleAccountBuilder {
    email: String,
    debug: Option<bool>,
    anisette_provider: Option<Box<dyn AnisetteProvider>>,
}

#[derive(Debug)]
pub enum LoginState {
    LoggedIn,
    NeedsDevice2FA,
    NeedsSMS2FA,
    NeedsExtraStep(String),
    NeedsLogin,
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
            anisette_provider: None,
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

    pub fn anisette_provider(mut self, anisette_provider: impl AnisetteProvider + 'static) -> Self {
        self.anisette_provider = Some(Box::new(anisette_provider));
        self
    }

    /// Build the AppleAccount
    ///
    /// # Errors
    /// Returns an error if the reqwest client cannot be built
    pub async fn build(self) -> Result<AppleAccount, Report> {
        let debug = self.debug.unwrap_or(false);
        let anisette_provider = self
            .anisette_provider
            .unwrap_or_else(|| Box::new(RemoteV3AnisetteProvider::default()));

        AppleAccount::new(&self.email, anisette_provider, debug).await
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
        let mut account = self.build().await?;
        account.login(password, two_factor_callback).await?;
        Ok(account)
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

    /// Build the apple account with the given email
    ///
    /// Reccomended to use the AppleAccountBuilder instead
    pub async fn new(
        email: &str,
        mut anisette_provider: Box<dyn AnisetteProvider>,
        debug: bool,
    ) -> Result<Self, Report> {
        info!("Initializing apple account");
        if debug {
            warn!("Debug mode enabled: this is a security risk!");
        }

        let client_info = anisette_provider
            .get_client_info()
            .await
            .context("Failed to get anisette client info")?;

        let mut grandslam_client = GrandSlam::new(client_info, debug);

        let anisette_data = anisette_provider
            .get_anisette_data(&mut grandslam_client)
            .await
            .context("Failed to get anisette data for login")?;

        Ok(AppleAccount {
            email: email.to_string(),
            spd: None,
            anisette_provider,
            anisette_data,
            grandslam_client,
            debug,
            login_state: LoginState::NeedsLogin,
        })
    }

    pub async fn login(
        &mut self,
        password: &str,
        two_factor_callback: impl Fn() -> Option<String>,
    ) -> Result<(), Report> {
        info!("Logging in to Apple ID: {}", self.email);
        if self.debug {
            warn!("Debug mode enabled: this is a security risk!");
        }

        self.login_state = self
            .login_inner(password)
            .await
            .context("Failed to log in to Apple ID")?;

        debug!("Initial login successful");

        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > 10 {
                bail!(
                    "Couldn't login after 10 attempts, aborting (current state: {:?})",
                    self.login_state
                );
            }
            match &self.login_state {
                LoginState::LoggedIn => {
                    info!("Successfully logged in to Apple ID");
                    return Ok(());
                }
                LoginState::NeedsDevice2FA => {
                    self.trusted_device_2fa(&two_factor_callback)
                        .await
                        .context("Failed to complete trusted device 2FA")?;
                    debug!("Trusted device 2FA completed, need to login again");
                    self.login_state = LoginState::NeedsLogin;
                }
                LoginState::NeedsSMS2FA => {
                    info!("SMS 2FA required");
                    self.sms_2fa(&two_factor_callback)
                        .await
                        .context("Failed to complete SMS 2FA")?;
                    debug!("SMS 2FA completed, need to login again");
                    self.login_state = LoginState::NeedsLogin;
                }
                LoginState::NeedsExtraStep(s) => {
                    info!("Additional authentication step required: {}", s);
                    if self.get_pet().is_err() {
                        bail!("Additional authentication required: {}", s);
                    }
                    self.login_state = LoginState::LoggedIn;
                }
                LoginState::NeedsLogin => {
                    debug!("Logging in again...");
                    self.login_state = self
                        .login_inner(password)
                        .await
                        .context("Failed to login again")?;
                }
            }
        }
    }

    pub fn get_name(&self) -> Result<(String, String), Report> {
        let spd = self
            .spd
            .as_ref()
            .ok_or_else(|| report!("SPD not available, cannot get name"))?;

        Ok((spd.get_string("fn")?, spd.get_string("ln")?))
    }

    fn get_pet(&self) -> Result<String, Report> {
        let spd = self
            .spd
            .as_ref()
            .ok_or_else(|| report!("SPD not available, cannot get pet"))?;

        let pet = spd
            .get_dict("t")?
            .get_dict("com.apple.gs.idms.pet")?
            .get_string("token")?;

        Ok(pet)
    }

    async fn trusted_device_2fa(
        &mut self,
        two_factor_callback: impl Fn() -> Option<String>,
    ) -> Result<(), Report> {
        debug!("Trusted device 2FA required");
        let request_code_url = self
            .grandslam_client
            .get_url("trustedDeviceSecondaryAuth")
            .await?;

        let submit_code_url = self.grandslam_client.get_url("validateCode").await?;

        self.grandslam_client
            .get(&request_code_url)?
            .headers(self.build_2fa_headers().await?)
            .send()
            .await
            .context("Failed to request trusted device 2fa")?
            .error_for_status()
            .context("Trusted device 2FA request failed")?;

        info!("Trusted device 2FA request sent");

        let code =
            two_factor_callback().ok_or_else(|| report!("No 2FA code provided, aborting"))?;

        let res = self
            .grandslam_client
            .get(&submit_code_url)?
            .headers(self.build_2fa_headers().await?)
            .header("security-code", code)
            .send()
            .await
            .context("Failed to submit trusted device 2fa code")?
            .error_for_status()
            .context("Trusted device 2FA code submission failed")?
            .text()
            .await
            .context("Failed to read trusted device 2FA response text")?;

        let plist: Dictionary = plist::from_bytes(res.as_bytes())
            .context("Failed to parse trusted device response plist")
            .attach_with(|| res.clone())?;
        plist
            .check_grandslam_error()
            .context("Trusted device 2FA rejected")?;

        Ok(())
    }

    async fn sms_2fa(
        &mut self,
        two_factor_callback: impl Fn() -> Option<String>,
    ) -> Result<(), Report> {
        debug!("SMS 2FA required");

        let request_code_url = self.grandslam_client.get_url("secondaryAuth").await?;

        self.grandslam_client
            .get_sms(&request_code_url)?
            .headers(self.build_2fa_headers().await?)
            .send()
            .await
            .context("Failed to request SMS 2FA")?
            .error_for_status()
            .context("SMS 2FA request failed")?;

        info!("SMS 2FA request sent");

        let code =
            two_factor_callback().ok_or_else(|| report!("No 2FA code provided, aborting"))?;

        let body = serde_json::json!({
            "securityCode": {
                "code": code
            },
            "phoneNumber": {
                "id": 1
            },
            "mode": "sms"
        });

        debug!("{}", body);

        let mut headers = self.build_2fa_headers().await?;
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json, text/javascript, */*; q=0.01"),
        );

        let res = self
            .grandslam_client
            .post("https://gsa.apple.com/auth/verify/phone/securitycode")?
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .context("Failed to submit SMS 2FA code")?
            .error_for_status()
            .context("SMS 2FA code submission failed")?
            .text()
            .await
            .context("Failed to read SMS 2FA response")?;

        debug!("SMS 2FA response: {}", res);

        Ok(())
    }

    async fn build_2fa_headers(&mut self) -> Result<HeaderMap, Report> {
        let mut headers = self.anisette_data.get_header_map(SERIAL_NUMBER.to_string());

        let spd = self
            .spd
            .as_ref()
            .ok_or_else(|| report!("SPD data not available, cannot build 2FA headers"))?;

        let adsid = spd
            .get_str("adsid")
            .context("Failed to build 2FA headers")?;
        let token = spd
            .get_str("GsIdmsToken")
            .context("Failed to build 2FA headers")?;
        let identity = BASE64_STANDARD.encode(format!("{}:{}", adsid, token));

        headers.insert(
            "X-Apple-Identity-Token",
            reqwest::header::HeaderValue::from_str(&identity)?,
        );

        Ok(headers)
    }

    async fn login_inner(&mut self, password: &str) -> Result<LoginState, Report> {
        let gs_service_url = self.grandslam_client.get_url("gsService").await?;

        debug!("GrandSlam service URL: {}", gs_service_url);

        let cpd = self
            .anisette_data
            .get_client_provided_data(SERIAL_NUMBER.to_string());

        let srp_client = SrpClient::<Sha256>::new(&G_2048);
        let a: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
        let a_pub = srp_client.compute_public_ephemeral(&a);

        let req1 = plist!(dict {
            "Header": {
                "Version": "1.0.1"
            },
            "Request": {
                "A2k": a_pub, // A2k = client public ephemeral
                "cpd": cpd.clone(), // cpd = client provided data
                "o": "init", // o = operation
                "ps": [ // ps = protocols supported
                    "s2k",
                    "s2k_fo"
                ],
                "u": self.email.clone(), // u = username
            }
        });

        debug!("Sending initial login request");

        let response = self
            .grandslam_client
            .plist_request(&gs_service_url, &req1, None)
            .await
            .context("Failed to send initial login request")?
            .check_grandslam_error()
            .context("GrandSlam error during initial login request")?;

        debug!("Login step 1 completed");

        let salt = response
            .get_data("s")
            .context("Failed to parse initial login response")?;
        let b_pub = response
            .get_data("B")
            .context("Failed to parse initial login response")?;
        let iters = response
            .get_signed_integer("i")
            .context("Failed to parse initial login response")?;
        let c = response
            .get_str("c")
            .context("Failed to parse initial login response")?;
        let selected_protocol = response
            .get_str("sp")
            .context("Failed to parse initial login response")?;

        debug!(
            "Selected SRP protocol: {}, iterations: {}",
            selected_protocol, iters
        );

        if selected_protocol != "s2k" && selected_protocol != "s2k_fo" {
            bail!("Unsupported SRP protocol selected: {}", selected_protocol);
        }

        let hashed_password = Sha256::digest(password.as_bytes());

        let password_hash = if selected_protocol == "s2k_fo" {
            hex::encode(&hashed_password).into_bytes()
        } else {
            hashed_password.to_vec()
        };

        let mut password_buf = [0u8; 32];
        pbkdf2::pbkdf2::<hmac::Hmac<Sha256>>(&password_hash, salt, iters as u32, &mut password_buf)
            .context("Failed to derive password using PBKDF2")?;

        let verifier: SrpClientVerifier<Sha256> = srp_client
            .process_reply(&a, &self.email.as_bytes(), &password_buf, salt, b_pub)
            .unwrap();

        let req2 = plist!(dict {
            "Header": {
                "Version": "1.0.1"
            },
            "Request": {
                "M1": verifier.proof().to_vec(), // A2k = client public ephemeral
                "c": c, // c = client proof from step 1
                "cpd": cpd, // cpd = client provided data
                "o": "complete", // o = operation
                "u": self.email.clone(), // u = username
            }
        });

        debug!("Sending proof login request");

        let response2 = self
            .grandslam_client
            .plist_request(&gs_service_url, &req2, None)
            .await
            .context("Failed to send proof login request")?
            .check_grandslam_error()
            .context("GrandSlam error during proof login request")?;

        debug!("Login step 2 response received, verifying server proof");

        let m2 = response2
            .get_data("M2")
            .context("Failed to parse proof login response")?;
        verifier
            .verify_server(m2)
            .map_err(|e| report!("Negotiation failed, server proof mismatch: {}", e))?;

        debug!("Server proof verified");

        let spd_encrypted = response2
            .get_data("spd")
            .context("Failed to get SPD from login response")?;

        let spd_decrypted = Self::decrypt_cbc(&verifier, &spd_encrypted)
            .context("Failed to decrypt SPD from login response")?;
        let spd: plist::Dictionary =
            plist::from_bytes(&spd_decrypted).context("Failed to parse decrypted SPD plist")?;

        self.spd = Some(spd);

        let status = response2
            .get_dict("Status")
            .context("Failed to parse proof login response")?;

        debug!("Login step 2 completed");

        if let Some(plist::Value::String(s)) = status.get("au") {
            return Ok(match s.as_str() {
                "trustedDeviceSecondaryAuth" => LoginState::NeedsDevice2FA,
                "secondaryAuth" => LoginState::NeedsSMS2FA,
                "repair" => LoginState::LoggedIn, // Just means that you don't have 2FA set up
                unknown => LoginState::NeedsExtraStep(unknown.to_string()),
            });
        }

        Ok(LoginState::LoggedIn)
    }

    fn create_session_key(usr: &SrpClientVerifier<Sha256>, name: &str) -> Result<Vec<u8>, Report> {
        Ok(Hmac::<Sha256>::new_from_slice(&usr.key())?
            .chain_update(name.as_bytes())
            .finalize()
            .into_bytes()
            .to_vec())
    }

    fn decrypt_cbc(usr: &SrpClientVerifier<Sha256>, data: &[u8]) -> Result<Vec<u8>, Report> {
        let extra_data_key = Self::create_session_key(usr, "extra data key:")?;
        let extra_data_iv = Self::create_session_key(usr, "extra data iv:")?;
        let extra_data_iv = &extra_data_iv[..16];

        Ok(
            cbc::Decryptor::<aes::Aes256>::new_from_slices(&extra_data_key, extra_data_iv)?
                .decrypt_padded_vec_mut::<Pkcs7>(&data)?,
        )
    }
}

impl std::fmt::Display for AppleAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Apple Account: ")?;
        match self.get_name() {
            Ok((first, last)) => write!(f, "{} {} ", first, last),
            Err(_) => Ok(()),
        }?;
        write!(f, "{} ({:?})", self.email, self.login_state)
    }
}
