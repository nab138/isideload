use thiserror::Error as ThisError;
use thiserror_context::{Context, impl_context};

pub mod anisette;
pub mod auth;

#[derive(Debug, ThisError)]
pub enum ErrorInner {
    #[error("Failed sending request: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Failed parsing plist: {0}")]
    Plist(#[from] plist::Error),

    #[error("Invalid Header: {0}")]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
}

impl_context!(Error(ErrorInner));

pub type SideloadResult<T> = std::result::Result<T, Error>;
