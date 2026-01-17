use thiserror::Error as ThisError;

pub mod anisette;
pub mod auth;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
