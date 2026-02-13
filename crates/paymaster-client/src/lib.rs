mod transaction;

use std::ops::Deref;
use std::time::Duration;

use starknet::core::types::Felt;
use thiserror::Error;

pub use paymaster_rpc::{
    BuildTransactionRequest, BuildTransactionResponse, DeployAndInvokeTransaction, DeployTransaction, DeploymentParameters, DirectInvokeParameters,
    ExecutableInvokeParameters, ExecutableTransactionParameters, ExecuteDirectRequest, ExecuteDirectResponse, ExecuteDirectTransactionParameters, ExecuteRequest,
    ExecuteResponse, ExecutionParameters, FeeEstimate, FeeMode, InvokeParameters, InvokeTransaction, TimeBounds, TipPriority, TokenPrice, TransactionParameters,
};
pub use transaction::{HasTransaction, NeedsTransaction, PreparedTransaction, TransactionBuilder, STRK_TOKEN};

#[derive(Error, Debug)]
pub enum Error {
    #[error("RPC error: {0}")]
    Rpc(#[from] paymaster_rpc::client::Error),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("configuration error: {0}")]
    Configuration(String),
}

/// Paymaster client wrapping the low-level RPC client.
///
/// Use [`Deref`] to access all native RPC methods (e.g. `client.health()`).
/// Use [`transaction()`](PaymasterClient::transaction) to start building a transaction.
///
/// # Example
///
/// ```ignore
/// let client = PaymasterClient::builder("https://sepolia.paymaster.avnu.fi/")
///     .api_key("my-key")
///     .build()?;
///
/// // High-level builder
/// client.transaction(account_address)
///     .call(transfer_call)
///     .sponsored()
///     .send(&wallet).await?;
///
/// // Low-level RPC via Deref
/// client.health().await?;
/// ```
#[derive(Clone)]
pub struct PaymasterClient {
    inner: paymaster_rpc::client::Client,
}

impl PaymasterClient {
    pub fn new(endpoint: &str) -> Self {
        Self {
            inner: paymaster_rpc::client::Client::new(endpoint),
        }
    }

    pub fn builder(endpoint: impl Into<String>) -> PaymasterClientBuilder {
        PaymasterClientBuilder {
            inner: paymaster_rpc::client::Client::builder(endpoint),
        }
    }

    /// Starts building a transaction for the given account address.
    pub fn transaction(&self, address: Felt) -> TransactionBuilder<'_, NeedsTransaction> {
        TransactionBuilder::new(&self.inner, address)
    }
}

impl Deref for PaymasterClient {
    type Target = paymaster_rpc::client::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct PaymasterClientBuilder {
    inner: paymaster_rpc::client::ClientBuilder,
}

impl PaymasterClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.inner = self.inner.api_key(api_key);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    pub fn build(self) -> Result<PaymasterClient, paymaster_rpc::client::Error> {
        Ok(PaymasterClient { inner: self.inner.build()? })
    }
}
