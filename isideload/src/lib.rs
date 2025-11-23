pub mod application;
pub mod bundle;
pub mod certificate;
pub mod developer_session;
pub mod device;
pub mod sideload;

use std::io::Error as IOError;

use apple_codesign::AppleCodesignError;
pub use icloud_auth::{AnisetteConfiguration, AppleAccount};

use developer_session::DeveloperTeam;
use idevice::IdeviceError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Authentication error {0}: {1}")]
    Auth(i64, String),
    #[error("Developer session error {0}: {1}")]
    DeveloperSession(i64, String),
    #[error("Error: {0}")]
    Generic(String),
    #[error("Failed to parse: {0}")]
    Parse(String),
    #[error("Invalid bundle: {0}")]
    InvalidBundle(String),
    #[error("Certificate error: {0}")]
    Certificate(String),
    #[error(transparent)]
    Filesystem(#[from] IOError),
    #[error(transparent)]
    IdeviceError(#[from] IdeviceError),
    #[error(transparent)]
    AppleCodesignError(#[from] Box<AppleCodesignError>),
}

pub trait SideloadLogger: Send + Sync {
    fn log(&self, message: &str);
    fn error(&self, error: &Error);
}

pub struct DefaultLogger;

impl SideloadLogger for DefaultLogger {
    fn log(&self, message: &str) {
        println!("{message}");
    }

    fn error(&self, error: &Error) {
        eprintln!("Error: {}", error);
    }
}

/// Sideload configuration options.
pub struct SideloadConfiguration<'a> {
    pub machine_name: String,
    pub logger: &'a dyn SideloadLogger,
    pub store_dir: std::path::PathBuf,
    pub revoke_cert: bool,
    pub force_sidestore: bool,
    pub skip_register_extensions: bool,
}

impl Default for SideloadConfiguration<'_> {
    fn default() -> Self {
        SideloadConfiguration::new()
    }
}

impl<'a> SideloadConfiguration<'a> {
    pub fn new() -> Self {
        SideloadConfiguration {
            machine_name: "isideload".to_string(),
            logger: &DefaultLogger,
            store_dir: std::env::current_dir().unwrap(),
            revoke_cert: false,
            force_sidestore: false,
            skip_register_extensions: true,
        }
    }

    /// An arbitrary machine name to appear on the certificate (e.x. "CrossCode")
    pub fn set_machine_name(mut self, machine_name: String) -> Self {
        self.machine_name = machine_name;
        self
    }

    /// Logger for reporting progress and errors
    pub fn set_logger(mut self, logger: &'a dyn SideloadLogger) -> Self {
        self.logger = logger;
        self
    }

    /// Directory used to store intermediate artifacts (profiles, certs, etc.). This directory will not be cleared at the end.
    pub fn set_store_dir(mut self, store_dir: std::path::PathBuf) -> Self {
        self.store_dir = store_dir;
        self
    }

    /// Whether or not to revoke the certificate immediately after installation
    #[deprecated(
        since = "0.1.0",
        note = "Certificates will now be placed in SideStore automatically so there is no need to revoke"
    )]
    pub fn set_revoke_cert(mut self, revoke_cert: bool) -> Self {
        self.revoke_cert = revoke_cert;
        self
    }

    /// Whether or not to treat the app as SideStore (fixes LiveContainer+SideStore issues)
    pub fn set_force_sidestore(mut self, force: bool) -> Self {
        self.force_sidestore = force;
        self
    }

    /// Whether or not to skip registering app extensions (save app IDs, default true)
    pub fn set_skip_register_extensions(mut self, skip: bool) -> Self {
        self.skip_register_extensions = skip;
        self
    }
}
