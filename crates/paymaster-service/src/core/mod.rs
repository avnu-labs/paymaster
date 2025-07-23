use thiserror::Error;

pub mod context;

mod tracing;
pub use tracing::Fmt;

#[derive(Error, Debug)]
pub enum Error {
    #[error("configuration error {0}")]
    Configuration(String),
}

impl From<Error> for paymaster_common::service::Error {
    fn from(value: Error) -> Self {
        Self::from(value)
    }
}
