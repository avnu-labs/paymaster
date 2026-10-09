mod configuration;
pub use configuration::{Configuration, RPCConfiguration, DEFAULT_PRIVACY_GAS_OVERHEAD};
use paymaster_execution::{Client as ExecutionClient, Error as ExecutionError, TransactionDuplicateFilter};
use paymaster_prices::Client as PriceClient;
use paymaster_sponsoring::Client as SponsoringClient;
use starknet::core::types::Felt;

use crate::Error;

#[derive(Clone)]
pub struct Context {
    pub configuration: Configuration,

    pub price: PriceClient,
    pub sponsoring: SponsoringClient,

    pub execution: ExecutionClient,
    pub transaction_filter: TransactionDuplicateFilter,
}

impl Context {
    pub fn new(configuration: Configuration) -> Self {
        Self {
            price: PriceClient::new(&configuration.price),
            sponsoring: SponsoringClient::new(&configuration.sponsoring),

            execution: ExecutionClient::new(&configuration.clone().into()),
            transaction_filter: TransactionDuplicateFilter::default(),

            configuration,
        }
    }

    /// Resolve the fee charged by the configured privacy pool. Fails when no privacy pool is configured
    pub async fn resolve_privacy_pool_fee(&self) -> Result<u128, Error> {
        if self.configuration.privacy_pool == Felt::ZERO {
            return Err(ExecutionError::PrivacyPoolNotWhitelisted.into());
        }

        Ok(self
            .execution
            .starknet
            .resolve_privacy_pool_fee(self.configuration.privacy_pool)
            .await?)
    }
}
