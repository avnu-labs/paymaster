use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("RPC error: {0}")]
    Rpc(#[from] paymaster_rpc::client::Error),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("configuration error: {0}")]
    Configuration(String),
}
