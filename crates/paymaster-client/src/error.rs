use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("RPC error {code}: {message}")]
    Rpc { code: i64, message: String, data: Option<serde_json::Value> },

    #[error("signing error: {0}")]
    Signing(String),

    #[error("configuration error: {0}")]
    Configuration(String),
}
