extern crate starknet as starknet_rust;

mod execution;

use std::cmp::max;
use std::collections::{HashMap, HashSet};

use ::starknet::core::types::{Felt, InvokeTransactionResult, NonZeroFelt};
use ::starknet::macros::felt;
pub use execution::*;

pub mod diagnostics;
pub mod privacy;
pub mod tokens;

#[cfg(feature = "testing")]
pub mod testing;

mod error;
mod starknet;

use diagnostics::DiagnosticClient;
pub use error::Error;
use paymaster_common::{measure_duration, metric};
use paymaster_prices::{Client as PriceClient, PriceConfiguration};
use paymaster_relayer::{LockedRelayer, RelayerManager, RelayerManagerConfiguration, RelayersConfiguration};
use paymaster_starknet::transaction::{Calls, EstimatedCalls};
use paymaster_starknet::{Configuration as StarknetConfiguration, ContractAddress, StarknetAccount, StarknetAccountConfiguration};
use thiserror::Error;
mod filter;

pub use filter::TransactionDuplicateFilter;

use crate::starknet::Client as Starknet;
use serde::{Deserialize, Serialize};

/// Mainnet STRK token address
fn default_strk_token_address() -> Felt {
    felt!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d")
}

/// Configuration for a single privacy pool
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyPoolConfiguration {
    /// Pool contract address
    pub pool_address: Felt,
    /// Fixed address receiving all Withdraws
    pub fee_recipient: Felt,
    /// Tokens accepted for fee payment from this pool
    pub accepted_gas_tokens: HashSet<Felt>,
    /// STRK token address (defaults to mainnet STRK, override for devnet)
    #[serde(default = "default_strk_token_address")]
    pub strk_token_address: Felt,
}

/// Privacy configuration containing pool definitions and fee settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyConfiguration {
    /// Map of pool_address → pool config
    pub pools: HashMap<Felt, PrivacyPoolConfiguration>,
    /// Fee overhead for privacy gasless txs (e.g., 0.5 = 50%)
    pub privacy_fee_overhead: f32,
}

/// Execution client configuration
#[derive(Clone, Debug)]
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

    pub privacy: Option<PrivacyConfiguration>,
}

impl From<Configuration> for RelayerManagerConfiguration {
    fn from(value: Configuration) -> Self {
        RelayerManagerConfiguration {
            starknet: value.starknet,
            gas_tank: value.gas_tank,
            supported_tokens: value.supported_tokens,
            relayers: value.relayers,
            price: value.price,
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
    privacy_fee_multiplier: f32,

    estimate_account: StarknetAccount,
    relayers: RelayerManager,

    privacy_pool_client: Option<privacy::PrivacyPoolClient>,

    pub diagnostic_client: DiagnosticClient,
}

impl Client {
    /// Creates a new client given a configuration
    pub fn new(configuration: &Configuration) -> Self {
        let starknet = Starknet::new(&configuration.starknet);
        Self {
            price: PriceClient::new(&configuration.price),

            max_fee_multiplier: configuration.max_fee_multiplier,
            provider_fee_multiplier: 1.0 + configuration.provider_fee_overhead,
            privacy_fee_multiplier: configuration
                .privacy
                .as_ref()
                .map(|p| 1.0 + p.privacy_fee_overhead)
                .unwrap_or(1.0),

            estimate_account: Starknet::new(&configuration.starknet).initialize_account(&configuration.estimate_account),
            relayers: RelayerManager::new(&configuration.clone().into()),

            privacy_pool_client: configuration
                .privacy
                .as_ref()
                .map(|_| privacy::PrivacyPoolClient::new(starknet.clone(), 128)),

            starknet,

            diagnostic_client: DiagnosticClient::new(configuration.starknet.chain_id),
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
    pub async fn estimate(&self, calls: &Calls, tip: TipPriority) -> Result<EstimatedCalls, Error> {
        let tip = self.get_tip(tip).await?;
        let result = calls.estimate(&self.estimate_account, Some(tip)).await?;

        Ok(result)
    }

    /// Get the tip value given a priority
    pub async fn get_tip(&self, tip: TipPriority) -> Result<u64, Error> {
        let tip: u64 = match tip {
            TipPriority::Slow => max(self.starknet.fetch_median_tip().await? - 5, 0),
            TipPriority::Normal => self.starknet.fetch_median_tip().await?,
            TipPriority::Fast => self.starknet.fetch_median_tip().await? + 5,
            TipPriority::Custom(tip) => tip,
        };
        Ok(tip)
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

    pub fn apply_privacy_fee_multiplier(&self, value: Felt) -> Felt {
        let multiplier = Felt::from((self.privacy_fee_multiplier * 1000.0) as u32);
        let divisor = NonZeroFelt::from_felt_unchecked(Felt::from(1000));

        (multiplier * value).floor_div(&divisor)
    }

    pub fn privacy_pool_client(&self) -> Option<&privacy::PrivacyPoolClient> {
        self.privacy_pool_client.as_ref()
    }

    pub fn get_relayer_manager(&self) -> &RelayerManager {
        &self.relayers
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use crate::testing::TestEnvironment;

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    async fn apply_max_fee_modifier_properly() {
        let test = TestEnvironment::new().await;
        let mut client = test.default_client();

        client.max_fee_multiplier = 2.5;

        let value = Felt::from(13400);
        let result = client.apply_max_fee_multiplier(value);

        assert_eq!(result, Felt::from(33500));
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
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

#[cfg(test)]
mod privacy_config_tests {
    use starknet::core::types::Felt;
    use starknet::macros::felt;

    use crate::{PrivacyConfiguration, PrivacyPoolConfiguration};

    mod deserialize {
        use super::*;

        #[test]
        fn should_parse_all_fields_when_complete_json() {
            // Given
            let json = r#"{
                "pools": {
                    "0x1234": {
                        "pool_address": "0x1234",
                        "fee_recipient": "0xabcd",
                        "accepted_gas_tokens": ["0x1111", "0x2222"],
                        "strk_token_address": "0x4718"
                    }
                },
                "privacy_fee_overhead": 0.5
            }"#;

            // When
            let config: PrivacyConfiguration = serde_json::from_str(json).unwrap();

            // Then
            assert_eq!(config.pools.len(), 1);
            assert_eq!(config.privacy_fee_overhead, 0.5);
            let pool = config.pools.get(&Felt::from(0x1234u64)).unwrap();
            assert_eq!(pool.fee_recipient, Felt::from(0xABCDu64));
            assert_eq!(pool.accepted_gas_tokens.len(), 2);
            assert_eq!(pool.strk_token_address, Felt::from(0x4718u64));
        }

        #[test]
        fn should_default_strk_address_to_mainnet_when_omitted() {
            // Given
            let json = r#"{
                "pool_address": "0x1234",
                "fee_recipient": "0xabcd",
                "accepted_gas_tokens": ["0x1111"]
            }"#;

            // When
            let pool: PrivacyPoolConfiguration = serde_json::from_str(json).unwrap();

            // Then
            assert_eq!(
                pool.strk_token_address,
                felt!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d")
            );
        }
    }
}
