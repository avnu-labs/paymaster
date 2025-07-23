use jsonrpsee::core::Serialize;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObject;
use paymaster_execution::Error as PaymasterExecutionError;
use paymaster_prices::Error as PriceError;
use paymaster_relayer::Error as RelayerError;
use paymaster_starknet::Error as StarknetError;
use serde::Deserialize;
use starknet::core::types::ContractExecutionError;
use thiserror::Error;
use tracing::error;

mod context;
pub use context::{Configuration, RPCConfiguration};

mod endpoint;
pub use endpoint::build::{
    BuildTransactionRequest, BuildTransactionResponse, DeployAndInvokeTransaction, DeployTransaction, FeeEstimate, InvokeParameters, InvokeTransaction,
    TransactionParameters,
};
pub use endpoint::common::{DeploymentParameters, ExecutionParameters, FeeMode, TimeBounds};
pub use endpoint::execute::{ExecutableInvokeParameters, ExecutableTransactionParameters, ExecuteRequest, ExecuteResponse};
pub use endpoint::token::TokenPrice;

mod middleware;

#[cfg(test)]
mod testing;

pub mod client;
pub mod server;

#[rpc(server, client)]
pub trait PaymasterAPI {
    #[method(name = "paymaster_health", with_extensions)]
    async fn health(&self) -> Result<bool, Error>;

    #[method(name = "paymaster_isAvailable", with_extensions)]
    async fn is_available(&self) -> Result<bool, Error>;

    #[method(name = "paymaster_buildTransaction", with_extensions)]
    async fn build_transaction(&self, params: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error>;

    #[method(name = "paymaster_executeTransaction", with_extensions)]
    async fn execute_transaction(&self, params: ExecuteRequest) -> Result<ExecuteResponse, Error>;

    #[method(name = "paymaster_getSupportedTokens", with_extensions)]
    async fn get_supported_tokens(&self) -> Result<Vec<TokenPrice>, Error>;
}

#[derive(Deserialize, Error, Debug)]
pub enum Error {
    #[error("service not available")]
    ServiceNotAvailable,

    #[error("x-paymaster-api-key is invalid")]
    InvalidAPIKey,

    #[error("token not supported")]
    TokenNotSupported,

    #[error("blacklisted calls")]
    BlacklistedCalls,

    #[error("invalid address")]
    InvalidAddress,

    #[error("class hash not supported")]
    ClassHashNotSupported,

    #[error("invalid deployment data")]
    InvalidDeploymentData,

    #[error("invalid time bounds")]
    InvalidTimeBounds,

    #[error("invalid signature")]
    InvalidSignature,

    #[error("max amount too low")]
    MaxAmountTooLow,

    #[error("{0:?}")]
    Execution(ContractExecutionError),
}

impl From<StarknetError> for Error {
    fn from(value: StarknetError) -> Self {
        match value {
            StarknetError::InvalidVersion => Self::InvalidDeploymentData,
            StarknetError::ContractNotFound => Self::InvalidAddress,
            StarknetError::Internal(_) => Self::Execution(ContractExecutionError::Message("Internal error".to_string())),
            StarknetError::Execution(error) => Self::Execution(error),
            e => Self::Execution(ContractExecutionError::Message(e.to_string())),
        }
    }
}

impl From<PriceError> for Error {
    fn from(_: PriceError) -> Self {
        Self::Execution(ContractExecutionError::Message("Internal price oracle error".to_string()))
    }
}

impl From<RelayerError> for Error {
    fn from(value: RelayerError) -> Self {
        Self::Execution(ContractExecutionError::Message(value.to_string()))
    }
}

impl From<PaymasterExecutionError> for Error {
    fn from(value: PaymasterExecutionError) -> Self {
        Self::Execution(ContractExecutionError::Message(value.to_string()))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ExecutionError {
    execution_error: ContractExecutionError,
}

impl<'a> From<Error> for ErrorObject<'a> {
    fn from(value: Error) -> Self {
        match value {
            Error::TokenNotSupported => ErrorObject::borrowed(151, "An error occurred (TOKEN_NOT_SUPPORTED)", None),
            Error::InvalidAddress => ErrorObject::borrowed(150, "An error occurred (INVALID_ADDRESS)", None),
            Error::InvalidSignature => ErrorObject::borrowed(153, "An error occurred (INVALID_SIGNATURE)", None),
            Error::MaxAmountTooLow => ErrorObject::borrowed(154, "An error occurred (MAX_AMOUNT_TOO_LOW)", None),
            Error::ClassHashNotSupported => ErrorObject::borrowed(155, "An error occurred (CLASS_HASH_NOT_SUPPORTED)", None),
            Error::InvalidTimeBounds => ErrorObject::borrowed(157, "An error occurred (INVALID_TIME_BOUNDS)", None),
            Error::InvalidDeploymentData => ErrorObject::borrowed(158, "An error occurred (INVALID_DEPLOYMENT_DATA)", None),
            Error::Execution(e) => ErrorObject::owned(156, "An error occurred (TRANSACTION_EXECUTION_ERROR)", Some(ExecutionError { execution_error: e })),
            Error::BlacklistedCalls => ErrorObject::owned(163, "An error occurred (UNKNOWN_ERROR)", Some(Error::BlacklistedCalls.to_string())),
            Error::ServiceNotAvailable => ErrorObject::owned(163, "An error occurred (UNKNOWN_ERROR)", Some(Error::ServiceNotAvailable.to_string())),
            Error::InvalidAPIKey => ErrorObject::owned(163, "An error occurred (UNKNOWN_ERROR)", Some(Error::InvalidAPIKey.to_string())),
        }
    }
}
