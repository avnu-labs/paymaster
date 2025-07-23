use std::time::Duration;

use paymaster_relayer::lock::LockLayerConfiguration;
use paymaster_sponsoring::Configuration as SponsoringConfiguration;

// Core configuration defaults
pub const DEFAULT_VERBOSITY: &str = "info";

// RPC configuration defaults
pub const DEFAULT_RPC_PORT: u64 = 12777;
pub const DEFAULT_STARKNET_TIMEOUT: u64 = 1;
pub const DEFAULT_MAX_CHECK_STATUS_ATTEMPTS: usize = 5;

// Paymaster configuration defaults
pub const DEFAULT_MAX_FEE_MULTIPLIER: f32 = 3.0;
pub const DEFAULT_SPONSORING_MODE: SponsoringConfiguration = SponsoringConfiguration::None;
pub const DEFAULT_PROVIDER_FEE_OVERHEAD: f32 = 0.1;

// Relayers configuration defaults
pub const DEFAULT_RELAYERS_NUM: usize = 2;
pub const DEFAULT_INITIAL_ESTIMATE_ACCOUNT_FUND_AMOUNT: f64 = 10.0;
pub const DEFAULT_INITIAL_GAS_TANK_FUND_AMOUNT: f64 = 30.0;
pub const DEFAULT_RELAYERS_RETRY_TIMEOUT: u64 = 1;
pub const DEFAULT_RELAYERS_LOCK_MODE: LockLayerConfiguration = LockLayerConfiguration::Seggregated {
    retry_timeout: Duration::from_secs(DEFAULT_RELAYERS_RETRY_TIMEOUT),
};

// Rebalancing configuration defaults
pub const DEFAULT_REBALANCING_CHECK_INTERVAL: u64 = 3600 * 24; // every 24 hours
pub const DEFAULT_MIN_RELAYER_BALANCE: f64 = 1.0; // 1 STRK
pub const DEFAULT_RELAYERS_REBALANCE_TRIGGER_AMOUNT: f64 = 8.0; // 2 STRK

// Swap params configuration defaults
pub const DEFAULT_MIN_SWAP_SELL_AMOUNT: f64 = 1.0;
pub const DEFAULT_MAX_PRICE_IMPACT: f64 = 0.05; // 5%
pub const DEFAULT_SWAP_INTERVAL: u64 = 3600; // every hour
pub const DEFAULT_SWAP_SLIPPAGE: f64 = 0.01; // 1%
