use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::accounts::{AccountError, ArgentAccountFactory, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::typed_data::TypedDataError;
use starknet::core::types::SimulationFlagForEstimateFee::SkipValidate;
use starknet::core::types::{
    BlockId, BlockTag, BroadcastedTransaction, ContractExecutionError, FeeEstimate, Felt, FunctionCall, MaybePreConfirmedBlockWithTxs, StarknetError, Transaction,
    TransactionReceiptWithBlockInfo, TransactionStatus,
};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};
use starknet::signers::{LocalWallet, SigningKey};
use thiserror::Error;
use tracing::{error, instrument};

pub mod constants;
pub mod contract;
pub mod math;
pub mod transaction;
pub mod types;
pub mod values;

mod gas;
pub use gas::BlockGasPrice;
pub use tracing;

mod network;
pub use network::ChainID;
use paymaster_common::service::fallback;
use paymaster_common::{measure_duration, metric};

use crate::client::StarknetClient;
use crate::constants::ClassHash;
use crate::contract::ContractClass;

#[cfg(feature = "testing")]
pub mod testing;

mod client;

pub type StarknetAccount = SingleOwnerAccount<StarknetClient, LocalWallet>;

#[macro_export]
macro_rules! log_if_error {
    ($e: expr) => {
        match $e {
            Ok(v) => Ok(v),
            Err(ProviderError::StarknetError(error)) => {
                $crate::tracing::warn!(message=%error);
                Err(ProviderError::StarknetError(error))
            }
            Err(error) => {
               $crate::tracing::error!(message=%error);
               Err(error)
            }
        }
    };
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct StarknetAccountConfiguration {
    #[serde_as(as = "UfeHex")]
    pub address: ContractAddress,

    #[serde_as(as = "UfeHex")]
    pub private_key: Felt,
}

pub type Signature = Vec<Felt>;
pub type ContractAddress = Felt;

#[derive(Error, Debug)]
pub enum Error {
    #[error("internal error {0}")]
    Internal(String),

    #[error("missing gas fee transfer")]
    MissingGasFeeTransferCall,

    #[error("invalid nonce {0}")]
    InvalidNonce(String),

    #[error("invalid paymaster version")]
    InvalidVersion,

    #[error("contract not found")]
    ContractNotFound,

    #[error("contract error {0}")]
    Contract(String),

    #[error("typed data encoding {0}")]
    TypedDataEncoding(#[from] TypedDataError),

    #[error("typed data decoding {0}")]
    TypedDataDecoding(String),

    #[error("starknet error {0}")]
    Starknet(String),

    #[error("Execution error {0:?}")]
    Execution(ContractExecutionError),

    #[error("starknet error {0}")]
    ValidationFailure(String),
}

impl From<ProviderError> for Error {
    fn from(value: ProviderError) -> Self {
        match value {
            ProviderError::StarknetError(StarknetError::InvalidTransactionNonce(value)) => Error::InvalidNonce(value),
            ProviderError::StarknetError(StarknetError::TransactionExecutionError(e)) => Error::Execution(e.execution_error),
            ProviderError::StarknetError(StarknetError::ContractError(e)) => Error::Execution(e.revert_error),
            ProviderError::StarknetError(StarknetError::ContractNotFound) => Error::ContractNotFound,
            ProviderError::StarknetError(StarknetError::ValidationFailure(error)) => Error::ValidationFailure(format!("ValidationFailure: {:?}", error)),
            ProviderError::Other(e) => Error::Internal(e.to_string()),
            ProviderError::RateLimited => Error::Internal("RateLimited".to_string()),
            ProviderError::ArrayLengthMismatch => Error::Internal("ArrayLengthMismatch".to_string()),
            e => Error::Starknet(e.to_string()),
        }
    }
}

impl From<fallback::Error<ProviderError>> for Error {
    fn from(value: fallback::Error<ProviderError>) -> Self {
        match value {
            fallback::Error::Rejected => Self::Internal("could not connect to endpoint".to_string()),
            fallback::Error::Inner(e) => e.into(),
        }
    }
}

impl<T: Display + Debug> From<AccountError<T>> for Error {
    fn from(value: AccountError<T>) -> Self {
        match value {
            AccountError::Provider(error) => error.into(),
            e => Error::Starknet(format!("{}", e)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub chain_id: ChainID,
    pub endpoint: String,
    pub timeout: u64,

    #[serde(default)]
    pub fallbacks: Vec<String>,
}

#[derive(Clone)]
pub struct Client {
    chain_id: ChainID,

    inner: StarknetClient,
}

impl Client {
    pub fn new(configuration: &Configuration) -> Self {
        let mut client = StarknetClient::new(&configuration.endpoint, configuration.timeout);
        for fallback in &configuration.fallbacks {
            client = client.with_fallback(fallback, configuration.timeout);
        }

        Self {
            chain_id: configuration.chain_id,
            inner: client,
        }
    }

    /// Returns the chain_id on which this client is bound
    pub fn chain_id(&self) -> &ChainID {
        &self.chain_id
    }

    /// Initialize an account using the given account configuration
    pub fn initialize_account(&self, account: &StarknetAccountConfiguration) -> StarknetAccount {
        let signing_key = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(account.private_key));

        let mut account = StarknetAccount::new(self.inner.clone(), signing_key, account.address, self.chain_id.as_felt(), ExecutionEncoding::New);
        account.set_block_id(BlockId::Tag(BlockTag::PreConfirmed));
        account
    }

    /// Initialize an argent account using the given account configuration.
    pub async fn initialize_argent_account(&self, private_key: Felt) -> ArgentAccountFactory<LocalWallet, StarknetClient> {
        let class_hash = ClassHash::ARGENT_ACCOUNT;
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(private_key));

        ArgentAccountFactory::new(class_hash, self.chain_id.as_felt(), None, signer, self.inner.clone())
            .await
            .unwrap()
    }

    /// Fetch the gas price at the latest block. Price is given in wei
    #[instrument(name = "fetch_block_gas_price", skip(self))]
    pub async fn fetch_block_gas_price(&self) -> Result<BlockGasPrice, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_block_with_txs(BlockId::Tag(BlockTag::Latest)).await));
        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "fetch_block_gas_price");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "fetch_block_gas_price");

        let prices = match result? {
            MaybePreConfirmedBlockWithTxs::Block(block) => (block.l1_gas_price.price_in_fri, block.l1_data_gas_price.price_in_fri),
            MaybePreConfirmedBlockWithTxs::PreConfirmedBlock(block) => (block.l1_gas_price.price_in_fri, block.l1_data_gas_price.price_in_fri),
        };

        Ok(BlockGasPrice {
            computation: prices.0,
            storage: prices.1,
        })
    }

    /// Fetch the median tip at the latest block
    #[instrument(name = "fetch_block_median_tip", skip(self))]
    pub async fn fetch_block_median_tip(&self) -> Result<u64, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_block_with_txs(BlockId::Tag(BlockTag::Latest)).await));
        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "fetch_block_median_tip");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "fetch_block_median_tip");
        Ok(result?.median_tip())
    }

    /// Call `balance_of(recipient)` on the given `token` address
    #[instrument(name = "fetch_balance", skip(self))]
    pub async fn fetch_balance(&self, token: Felt, recipient: Felt) -> Result<Felt, Error> {
        let call = FunctionCall {
            contract_address: token,
            entry_point_selector: selector!("balance_of"),
            calldata: vec![recipient],
        };

        let (result, duration) = measure_duration!(log_if_error!(self.inner.call(call, BlockId::Tag(BlockTag::PreConfirmed)).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "token_balance_of");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "token_balance_of");

        result?.first().cloned().ok_or(Error::ContractNotFound)
    }

    /// Fetch the nonce of the given `user`
    #[instrument(name = "fetch_nonce", skip(self))]
    pub async fn fetch_nonce(&self, user: ContractAddress) -> Result<Felt, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_nonce(BlockId::Tag(BlockTag::PreConfirmed), user).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "get_nonce");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "get_nonce");

        Ok(result?)
    }

    /// Execute the given `call`
    #[instrument(name = "call", skip(self))]
    pub async fn call(&self, call: &FunctionCall) -> Result<Vec<Felt>, Error> {
        let block = BlockId::Tag(BlockTag::PreConfirmed);
        let (result, duration) = measure_duration!(log_if_error!(self.inner.call(call, block).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "call");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "call");

        Ok(result?)
    }

    /// Estimates the `transactions` and returns their [`FeeEstimate`]
    #[instrument(name = "estimate_transactions", skip(self))]
    pub async fn estimate_transactions(&self, transactions: &[BroadcastedTransaction]) -> Result<Vec<FeeEstimate>, Error> {
        let block = BlockId::Tag(BlockTag::PreConfirmed);

        // Estimate fees
        let (result, duration) = measure_duration!(log_if_error!(self.inner.estimate_fee(transactions, vec![SkipValidate], block).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "estimate_transactions");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "estimate_transactions");

        Ok(result?)
    }

    /// Returns the receipt of the transaction with `hash`
    #[instrument(name = "fetch_class", skip(self))]
    pub async fn fetch_class(&self, class_hash: Felt) -> Result<ContractClass, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_class(BlockId::Tag(BlockTag::Latest), class_hash).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "get_class");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "get_class");

        Ok(ContractClass::from_class(result?))
    }

    /// Returns the receipt of the transaction with `hash`
    #[instrument(name = "get_transaction_receipt", skip(self))]
    pub async fn get_transaction_receipt(&self, hash: Felt) -> Result<TransactionReceiptWithBlockInfo, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_transaction_receipt(hash).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "get_transaction_receipt");
        metric!(on error result => counter [starknet_rpc_error ] = 1, method = "get_transaction_receipt");

        Ok(result?)
    }

    /// Returns the status of the transaction with `hash`
    #[instrument(name = "get_transaction_status", skip(self))]
    pub async fn get_transaction_status(&self, hash: Felt) -> Result<TransactionStatus, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_transaction_status(hash).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "get_transaction_status");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "get_transaction_status");

        Ok(result?)
    }

    /// Returns the transaction with `hash`
    #[instrument(name = "get_transaction", skip(self))]
    pub async fn get_transaction(&self, hash: Felt) -> Result<Transaction, Error> {
        let (result, duration) = measure_duration!(log_if_error!(self.inner.get_transaction_by_hash(hash).await));

        metric!(histogram[starknet_rpc] = duration.as_millis(), method = "get_transaction");
        metric!(on error result => counter [ starknet_rpc_error ] = 1, method = "get_transaction");

        Ok(result?)
    }
}
