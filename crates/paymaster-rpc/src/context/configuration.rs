use std::collections::HashSet;

use paymaster_prices::PriceConfiguration;
use paymaster_relayer::RelayersConfiguration;
use paymaster_sponsoring::Configuration as SponsoringConfiguration;
use paymaster_starknet::{Configuration as StarknetConfiguration, StarknetAccountConfiguration};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::types::Felt;

/// Default L2 gas overhead for privacy pool execution (proof verification, forwarder, etc.).
pub const DEFAULT_PRIVACY_GAS_OVERHEAD: u64 = 80_000_000;

#[derive(Clone, Debug)]
pub struct Configuration {
    pub rpc: RPCConfiguration,

    pub forwarder: Felt,
    pub supported_tokens: HashSet<Felt>,

    pub privacy_pool: Felt,
    pub privacy_pool_fee_amount: u128,
    pub privacy_gas_overhead: u64,

    pub max_fee_multiplier: f32,
    pub provider_fee_overhead: f32,

    pub estimate_account: StarknetAccountConfiguration,
    pub gas_tank: StarknetAccountConfiguration,

    pub relayers: RelayersConfiguration,

    pub starknet: StarknetConfiguration,
    pub price: PriceConfiguration,
    pub sponsoring: SponsoringConfiguration,
}

impl From<Configuration> for paymaster_execution::Configuration {
    fn from(value: Configuration) -> Self {
        Self {
            starknet: value.starknet,
            price: value.price,
            supported_tokens: value.supported_tokens,
            max_fee_multiplier: value.max_fee_multiplier,
            provider_fee_overhead: value.provider_fee_overhead,

            estimate_account: value.estimate_account,
            gas_tank: value.gas_tank,

            relayers: value.relayers,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RPCConfiguration {
    pub port: u64,
}
