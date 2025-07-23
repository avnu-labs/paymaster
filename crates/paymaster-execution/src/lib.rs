mod execution;
use std::collections::HashSet;

pub use execution::*;

#[cfg(feature = "testing")]
pub mod testing;

mod error;
mod starknet;
use ::starknet::core::types::{Felt, InvokeTransactionResult, NonZeroFelt};
pub use error::Error;
use paymaster_common::{measure_duration, metric};
use paymaster_prices::{Client as PriceClient, Configuration as PriceConfiguration};
use paymaster_relayer::{LockedRelayer, RelayerManager, RelayerManagerConfiguration, RelayersConfiguration};
use paymaster_starknet::transaction::{Calls, EstimatedCalls};
use paymaster_starknet::{Configuration as StarknetConfiguration, ContractAddress, StarknetAccount, StarknetAccountConfiguration};
use serde::Deserialize;
use thiserror::Error;

use crate::starknet::Client as Starknet;

/// Execution client configuration
#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    /// Account used to estimate transaction. This account should only be used
    /// for estimate purpose and its nonce must never change. In the case where
    /// the nonce is changed, the system will pick up the proper nonce, but it could
    /// result in several failed estimation.
    pub estimate_account: StarknetAccountConfiguration,

    /// Account used to receive the fee in gas token.
    pub gas_tank: StarknetAccountConfiguration,

    /// Multiply the estimated fee by this factor to produce the maximum amount of fee
    /// we expect the user to pay. When the transaction is built, the user must approve
    /// the maximum fee amount the larger the multiplier the larger the approve.
    ///
    /// A good value based on empirical experiment is 3.
    pub max_fee_multiplier: f32,

    /// Provider overhead which is added to the fee paid by the user. The fee paid by the user
    /// are computed as (1.0 + provider_overhead) * fee_estimate.
    pub provider_fee_overhead: f32,

    pub supported_tokens: HashSet<Felt>,

    pub starknet: StarknetConfiguration,
    pub price: PriceConfiguration,

    pub relayers: RelayersConfiguration,
}

impl From<Configuration> for RelayerManagerConfiguration {
    fn from(value: Configuration) -> Self {
        RelayerManagerConfiguration {
            starknet: value.starknet,
            gas_tank: value.gas_tank,
            supported_tokens: value.supported_tokens,
            relayers: value.relayers,
        }
    }
}

/// Execution client exposing the function to estimate and execute paymaster transactions
#[derive(Clone)]
pub struct Client {
    pub starknet: Starknet,
    pub price: PriceClient,

    max_fee_multiplier: f32,
    provider_fee_multiplier: f32,

    estimate_account: StarknetAccount,
    relayers: RelayerManager,
}

impl Client {
    /// Creates a new client given a configuration
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            starknet: Starknet::new(&configuration.starknet),
            price: PriceClient::new(&configuration.price),

            max_fee_multiplier: configuration.max_fee_multiplier,
            provider_fee_multiplier: 1.0 + configuration.provider_fee_overhead,

            estimate_account: Starknet::new(&configuration.starknet).initialize_account(&configuration.estimate_account),
            relayers: RelayerManager::new(&configuration.clone().into()),
        }
    }

    /// Execute the calls after they have been estimated. See method [`estimate`]
    pub async fn execute(&self, calls: &EstimatedCalls) -> Result<InvokeTransactionResult, Error> {
        let mut relayer = self.relayers.lock_relayer().await?;

        let (result, duration) = measure_duration!(self.execute_with_retries(&mut relayer, calls, 3).await);
        metric!(counter[execution_request] = 1, method = "execute");
        metric!(histogram[execution_request_duration_milliseconds] = duration.as_millis(), method = "execute");

        match result {
            Ok(result) => {
                let _ = self.relayers.release_relayer(relayer).await;

                Ok(result)
            },
            Err(Error::InvalidNonce) => {
                metric!(counter[execution_request_error] = 1, method = "execute", error = "invalid_nonce");
                let _ = self.relayers.release_relayer_delayed(relayer, 20).await;

                Err(Error::InvalidNonce)
            },
            Err(e) => {
                metric!(counter[execution_request_error] = 1, method = "execute", error = e.to_string());
                let _ = self.relayers.release_relayer(relayer).await;

                Err(e)
            },
        }
    }

    // Execute the transaction at most n times in the case where it fails because of an invalid nonce.
    // Note that if the transaction fails for a differant reason than an invalid nonce, this function returns the
    // error.
    async fn execute_with_retries(&self, relayer: &mut LockedRelayer, calls: &EstimatedCalls, n_retries: usize) -> Result<InvokeTransactionResult, Error> {
        for _ in 0..n_retries {
            match relayer.execute(calls).await {
                Ok(result) => return Ok(result),
                Err(paymaster_relayer::Error::InvalidNonce) => {},
                Err(e) => return Err(Error::Execution(e.to_string())),
            }
        }

        Err(Error::InvalidNonce)
    }

    /// Estimate the gas cost of a sequence of calls using the account configured for estimation
    pub async fn estimate(&self, calls: &Calls) -> Result<EstimatedCalls, Error> {
        let result = calls.estimate(&self.estimate_account).await?;

        Ok(result)
    }

    pub fn compute_max_fee_in_strk(&self, base_estimate: Felt) -> Felt {
        self.apply_max_fee_multiplier(self.compute_fee_in_strk(base_estimate))
    }

    pub async fn compute_max_fee_with_overhead_in_strk(&self, user: ContractAddress, base_estimate: Felt) -> Result<Felt, Error> {
        self.compute_fee_with_overhead_in_strk(user, base_estimate)
            .await
            .map(|x| self.apply_max_fee_multiplier(x))
    }

    pub fn compute_paid_fee_in_strk(&self, base_estimate: Felt) -> Felt {
        self.apply_provider_fee_multiplier(self.compute_fee_in_strk(base_estimate))
    }

    pub async fn compute_paid_fee_with_overhead_in_strk(&self, user: ContractAddress, base_estimate: Felt) -> Result<Felt, Error> {
        self.compute_fee_with_overhead_in_strk(user, base_estimate)
            .await
            .map(|x| self.apply_provider_fee_multiplier(x))
    }

    /// Compute the fee in strk given [`base_estimate`] which corresponds to the original estimate in strk
    fn compute_fee_in_strk(&self, base_estimate: Felt) -> Felt {
        base_estimate
    }

    /// Compute the fee in strk given [`base_estimate`] which corresponds to the original estimate in strk on top of
    /// which we add the approximate overhead induced by the [`user`] account type.
    async fn compute_fee_with_overhead_in_strk(&self, user: ContractAddress, base_estimate: Felt) -> Result<Felt, Error> {
        let gas_price = self.starknet.fetch_block_gas_price().await?;

        let overhead = self.starknet.resolve_gas_overhead(user).await?;
        let overhead_estimate = gas_price * overhead;

        Ok(self.compute_fee_in_strk(base_estimate) + overhead_estimate)
    }

    fn apply_max_fee_multiplier(&self, value: Felt) -> Felt {
        let multiplier = Felt::from((self.max_fee_multiplier * 1000.0) as u32);
        let divisor = NonZeroFelt::from_felt_unchecked(Felt::from(1000));

        (multiplier * value).floor_div(&divisor)
    }

    fn apply_provider_fee_multiplier(&self, value: Felt) -> Felt {
        let multiplier = Felt::from((self.provider_fee_multiplier * 1000.0) as u32);
        let divisor = NonZeroFelt::from_felt_unchecked(Felt::from(1000));

        (multiplier * value).floor_div(&divisor)
    }

    pub fn get_relayer_manager(&self) -> &RelayerManager {
        &self.relayers
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use crate::testing::TestEnvironment;

    #[tokio::test]
    async fn apply_max_fee_modifier_properly() {
        let test = TestEnvironment::new().await;
        let mut client = test.default_client();

        client.max_fee_multiplier = 2.5;

        let value = Felt::from(13400);
        let result = client.apply_max_fee_multiplier(value);

        assert_eq!(result, Felt::from(33500));
    }

    #[tokio::test]
    async fn apply_provider_fee_overhead_properly() {
        let test = TestEnvironment::new().await;
        let mut client = test.default_client();

        client.provider_fee_multiplier = 0.25;

        let value = Felt::from(13400);
        let result = client.apply_provider_fee_multiplier(value);

        assert_eq!(result, Felt::from(3350));
    }
}
