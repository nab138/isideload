pub mod application;
pub mod bundle;
pub mod certificate;
pub mod developer_session;
pub mod device;
pub mod sideload;

use std::io::Error as IOError;

pub use developer_session::{
    AppId, ApplicationGroup, DeveloperDevice, DeveloperDeviceType, DeveloperSession, DeveloperTeam,
    DevelopmentCertificate, ListAppIdsResponse, ProvisioningProfile,
};
pub use icloud_auth::{AnisetteConfiguration, AppleAccount};

use idevice::IdeviceError;
use thiserror::Error as ThisError;
use zsign_rust::ZSignError;

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
    ZSignError(#[from] ZSignError),
}

pub trait SideloadLogger {
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
