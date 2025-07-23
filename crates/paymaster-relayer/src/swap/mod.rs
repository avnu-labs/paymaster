pub mod client;

pub use client::{SwapClient, SwapClientConfigurator};
use paymaster_common::service::Error as ServiceError;
use serde::{Deserialize, Serialize};

// Configuration for swap service
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapConfiguration {
    // Maximum slippage percentage for swaps (e.g., 0.01 for 1%)
    pub slippage: f64,
    // Swap client configuration (AVNU, Mock, etc.)
    pub swap_client_config: SwapClientConfigurator,
    // Maximum acceptable price impact as a decimal (e.g., 0.05 for 5%)
    pub max_price_impact: f64,
    // How often to check relayer balances (in seconds)
    pub swap_interval: u64,
    // Minimum sell value for a swap (in USD)
    pub min_usd_sell_amount: f64,
}

impl SwapConfiguration {
    // Validates the configuration parameters
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.slippage < 0.0 || self.slippage > 1.0 {
            return Err(ServiceError::new("slippage must be between 0.0 and 1.0"));
        }
        if self.max_price_impact < 0.0 || self.max_price_impact > 1.0 {
            return Err(ServiceError::new("Max price impact must be between 0.0 and 1.0"));
        }
        if self.min_usd_sell_amount <= 0.0 {
            return Err(ServiceError::new("min_usd_sell_amount must be greater than 0.0"));
        }
        self.swap_client_config.validate()
    }

    /// Create a swap client from this configuration
    pub fn create_client(&self) -> SwapClient {
        SwapClient::new(&self.swap_client_config)
    }
}
