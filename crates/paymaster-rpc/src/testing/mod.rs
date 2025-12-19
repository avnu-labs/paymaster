use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_prices::mock::MockPriceOracle;
use paymaster_prices::TokenPrice;
use paymaster_relayer::lock::mock::MockLockLayer;
use paymaster_relayer::lock::{LockLayerConfiguration, RelayerLock};
use paymaster_relayer::RelayersConfiguration;
use paymaster_starknet::constants::Token;
pub use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
use paymaster_starknet::StarknetAccountConfiguration;
use starknet::core::types::Felt;
use starknet::macros::felt;

use crate::context::{Context, RPCConfiguration};
use crate::Configuration;

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

    async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, paymaster_prices::Error> {
        Ok(TokenPrice {
            address,
            price_in_strk: Felt::from(1e18 as u128),
            decimals: 18,
        })
    }
}

#[derive(Debug)]
struct LockingLayer;

#[async_trait]
impl MockLockLayer for LockingLayer {
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
        Ok(RelayerLock::new(StarknetTestEnvironment::ACCOUNT_3.address, None, Duration::from_secs(30)))
    }

    async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), paymaster_relayer::lock::Error> {
        Ok(())
    }
}

pub struct TestEnvironment {
    context: Context,

    #[allow(dead_code)]
    starknet: StarknetTestEnvironment,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let starknet = StarknetTestEnvironment::new().await;

        let configuration = Configuration {
            rpc: RPCConfiguration { port: 12777 },

            supported_tokens: HashSet::from([Token::ETH_ADDRESS, Token::usdc(starknet.chain_id()).address]),
            forwarder: StarknetTestEnvironment::FORWARDER,
            gas_tank: StarknetAccountConfiguration {
                address: StarknetTestEnvironment::FORWARDER,
                private_key: felt!("0x0"),
            },

            max_fee_multiplier: 3.0,
            provider_fee_overhead: 0.1,

            estimate_account: StarknetAccountConfiguration {
                address: felt!("0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691"),
                private_key: felt!("0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9"),
            },

            relayers: RelayersConfiguration {
                private_key: StarknetTestEnvironment::ACCOUNT_3.private_key,
                addresses: vec![StarknetTestEnvironment::ACCOUNT_3.address],

                min_relayer_balance: Felt::ZERO,

                lock: LockLayerConfiguration::Mock {
                    retry_timeout: Duration::from_secs(5),
                    lock_layer: Arc::new(LockingLayer),
                },
                rebalancing: paymaster_relayer::rebalancing::OptionalRebalancingConfiguration::initialize(None),
            },

            starknet: starknet.configuration(),
            price: paymaster_prices::PriceConfiguration {
                principal: paymaster_prices::PriceOracleConfiguration::Mock(Arc::new(PriceOracle)),
                fallbacks: vec![],
            },
            sponsoring: paymaster_sponsoring::Configuration::none(),
        };

        Self {
            context: Context::new(configuration),

            starknet,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}
