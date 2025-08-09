pub mod application;
pub mod bundle;
pub mod certificate;
pub mod developer_session;
pub mod device;
pub mod sideload;

pub use developer_session::{
    AppId, ApplicationGroup, DeveloperDevice, DeveloperDeviceType, DeveloperSession, DeveloperTeam,
    DevelopmentCertificate, ListAppIdsResponse, ProvisioningProfile,
};

use thiserror::Error as ThisError;

#[derive(Debug, Clone, ThisError)]
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
    #[error("Failed to use files: {0}")]
    Filesystem(String),
}

pub trait SideloadLogger {
    async fn log(&self, message: &str);
    async fn error(&self, error: &Error);
}

pub struct DefaultLogger;

impl SideloadLogger for DefaultLogger {
    async fn log(&self, message: &str) {
        println!("{message}");
    }

    async fn error(&self, error: &Error) {
        eprintln!("Error: {}", error);
    }
}
