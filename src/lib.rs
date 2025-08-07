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

#[derive(Debug)]
pub enum Error {
    Auth(i64, String),
    DeveloperSession(i64, String),
    Generic,
    Parse,
    InvalidBundle(String),
    Certificate(String),
}
