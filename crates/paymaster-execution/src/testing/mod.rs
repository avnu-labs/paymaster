pub mod transaction;

use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_prices::mock::MockPriceOracle;
use paymaster_prices::{Error, TokenPrice};
use paymaster_relayer::lock::mock::MockLockLayer;
use paymaster_relayer::lock::{LockLayerConfiguration, RelayerLock};
use paymaster_relayer::RelayersConfiguration;
use paymaster_starknet::constants::Token;
pub use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
use starknet::core::types::Felt;

use crate::{Client, Configuration};

#[derive(Debug, Clone)]
struct PriceOracle;

#[async_trait]
impl MockPriceOracle for PriceOracle {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    async fn fetch_token(&self, _address: Felt) -> Result<TokenPrice, Error> {
        Ok(TokenPrice {
            address: Felt::ZERO,
            price_in_strk: Felt::from(1e18 as u128),
            decimals: 18,
        })
    }
}

#[derive(Debug)]
struct CoordinationLayer;

#[async_trait]
impl MockLockLayer for CoordinationLayer {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    async fn count_enabled_relayers(&self) -> usize {
        1
    }

    async fn set_enabled_relayers(&self, _relayers: &HashSet<Felt>) {}

    async fn lock_relayer(&self) -> Result<RelayerLock, paymaster_relayer::lock::Error> {
        Ok(RelayerLock::new(StarknetTestEnvironment::ACCOUNT_2.address, None, Duration::from_secs(5)))
    }

    async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), paymaster_relayer::lock::Error> {
        Ok(())
    }
}

pub struct TestEnvironment {
    pub configuration: Configuration,

    pub starknet: StarknetTestEnvironment,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let starknet = StarknetTestEnvironment::new().await;

        Self {
            configuration: Configuration {
                starknet: starknet.configuration(),
                price: paymaster_prices::Configuration::mock::<PriceOracle>(),
                supported_tokens: HashSet::from([Token::usdc(starknet.chain_id()).address]),
                max_fee_multiplier: 3.0,
                provider_fee_overhead: 0.1,

                estimate_account: StarknetTestEnvironment::ACCOUNT_1,
                gas_tank: StarknetTestEnvironment::ACCOUNT_1,

                relayers: RelayersConfiguration {
                    private_key: StarknetTestEnvironment::ACCOUNT_2.private_key,
                    addresses: vec![StarknetTestEnvironment::ACCOUNT_2.address],

                    min_relayer_balance: Felt::ZERO,
                    lock: LockLayerConfiguration::mock_with_timeout::<CoordinationLayer>(Duration::from_secs(5)),
                    rebalancing: paymaster_relayer::rebalancing::OptionalRebalancingConfiguration::initialize(None),
                },
            },

            starknet,
        }
    }

    pub fn default_client(&self) -> Client {
        Client::new(&self.configuration)
    }
}
