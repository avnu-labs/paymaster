mod transaction;

use thiserror::Error;

pub use paymaster_rpc::client::{Client as PaymasterClient, ClientBuilder as PaymasterClientBuilder};
pub use paymaster_rpc::{
    BuildTransactionRequest, BuildTransactionResponse, DeployAndInvokeTransaction, DeployTransaction, DeploymentParameters, DirectInvokeParameters,
    ExecutableInvokeParameters, ExecutableTransactionParameters, ExecuteDirectRequest, ExecuteDirectResponse, ExecuteDirectTransactionParameters, ExecuteRequest,
    ExecuteResponse, ExecutionParameters, FeeEstimate, FeeMode, InvokeParameters, InvokeTransaction, TimeBounds, TipPriority, TokenPrice, TransactionParameters,
};
pub use transaction::{PreparedTransaction, TransactionBuilder, STRK_TOKEN};

#[derive(Error, Debug)]
pub enum Error {
    #[error("RPC error: {0}")]
    Rpc(#[from] paymaster_rpc::client::Error),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("configuration error: {0}")]
    Configuration(String),
}
