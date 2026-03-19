use starknet::core::types::{FeeEstimate, Felt};

use crate::gas::BlockGasPrice;
use crate::Error;

#[derive(Debug, Clone)]
pub struct TransactionGasEstimate {
    pub overall_fee: u128,
    tip: u64,
    l1_gas_consumed: u64,
    l1_gas_price: u128,
    l2_gas_consumed: u64,
    l2_gas_price: u128,
    l1_data_gas_consumed: u64,
    l1_data_gas_price: u128,
    gas_estimate_multiplier: f64,
    gas_price_estimate_multiplier: f64,
}

impl TransactionGasEstimate {
    pub fn new(estimate: FeeEstimate, tip: u64) -> Self {
        Self {
            overall_fee: estimate.overall_fee,
            l1_gas_price: estimate.l1_gas_price,
            l2_gas_price: estimate.l2_gas_price,
            l1_data_gas_price: estimate.l1_data_gas_price,
            l1_gas_consumed: estimate.l1_gas_consumed,
            l2_gas_consumed: estimate.l2_gas_consumed,
            l1_data_gas_consumed: estimate.l1_data_gas_consumed,
            tip,
            gas_estimate_multiplier: 1.5,
            gas_price_estimate_multiplier: 1.5,
        }
    }

    /// Create a gas estimate from block gas prices with fixed generous gas consumption.
    /// Used for privacy pool transactions where fee estimation via simulation is not
    /// supported by the node (proof_facts are rejected during starknet_estimateFee).
    pub fn from_block_gas_prices(gas_prices: BlockGasPrice, tip: u64) -> Result<Self, Error> {
        // Fixed generous gas consumption for privacy pool transactions.
        // Based on observed actual consumption: ~80M l2_gas, ~2000 l1_data_gas.
        const L2_GAS_CONSUMED: u64 = 200_000_000;
        const L1_GAS_CONSUMED: u64 = 0;
        const L1_DATA_GAS_CONSUMED: u64 = 5_000;

        let l1_gas_price = crate::math::felt_to_u128(gas_prices.l1_gas_price)?;
        let l2_gas_price = crate::math::felt_to_u128(gas_prices.l2_gas_price)?;
        let l1_data_gas_price = crate::math::felt_to_u128(gas_prices.l1_data_gas_price)?;

        let overall_fee = L1_GAS_CONSUMED as u128 * l1_gas_price + L2_GAS_CONSUMED as u128 * l2_gas_price + L1_DATA_GAS_CONSUMED as u128 * l1_data_gas_price;

        Ok(Self {
            overall_fee,
            tip,
            l1_gas_consumed: L1_GAS_CONSUMED,
            l1_gas_price,
            l2_gas_consumed: L2_GAS_CONSUMED,
            l2_gas_price,
            l1_data_gas_consumed: L1_DATA_GAS_CONSUMED,
            l1_data_gas_price,
            gas_estimate_multiplier: 1.5,
            gas_price_estimate_multiplier: 1.5,
        })
    }

    pub fn update_overall_fee(self, overall_fee: Felt) -> Self {
        // Calculate the L2 gas consumed based on the overall fee and the L1 gas and data gas consumed
        // The new overall fee includes validation headers. The validation overhead only applies to l2_gas_consumed
        let overall_fee_u128: u128 = overall_fee.try_into().unwrap_or(self.overall_fee);
        let l2_gas_consumed = if self.l2_gas_consumed != 0 {
            ((overall_fee_u128 - (self.l1_gas_consumed as u128 * self.l1_gas_price + self.l1_data_gas_consumed as u128 * self.l1_data_gas_price)) / self.l2_gas_price)
                as u64
        } else {
            self.l2_gas_consumed
        };
        Self {
            overall_fee: overall_fee_u128,
            l1_gas_price: self.l1_gas_price,
            l2_gas_price: self.l2_gas_price,
            l1_data_gas_price: self.l1_data_gas_price,
            l1_gas_consumed: self.l1_gas_consumed,
            l2_gas_consumed,
            tip: self.tip,
            l1_data_gas_consumed: self.l1_data_gas_consumed,
            gas_estimate_multiplier: self.gas_estimate_multiplier,
            gas_price_estimate_multiplier: self.gas_price_estimate_multiplier,
        }
    }

    pub fn tip(&self) -> u64 {
        self.tip
    }

    pub fn l1_gas_consumed(&self) -> u64 {
        ((self.l1_gas_consumed as f64) * self.gas_estimate_multiplier) as u64
    }

    pub fn l2_gas_consumed(&self) -> u64 {
        ((self.l2_gas_consumed as f64) * self.gas_estimate_multiplier) as u64
    }

    pub fn l1_data_gas_consumed(&self) -> u64 {
        ((self.l1_data_gas_consumed as f64) * self.gas_estimate_multiplier) as u64
    }

    pub fn l1_gas_price(&self) -> Result<u128, Error> {
        Ok(
            ((TryInto::<u64>::try_into(self.l1_gas_price).map_err(|_| Error::Internal("Fee out of range".to_string()))? as f64) * self.gas_price_estimate_multiplier)
                as u128,
        )
    }

    pub fn l2_gas_price(&self) -> Result<u128, Error> {
        Ok(
            ((TryInto::<u64>::try_into(self.l2_gas_price).map_err(|_| Error::Internal("Fee out of range".to_string()))? as f64) * self.gas_price_estimate_multiplier)
                as u128,
        )
    }

    pub fn l1_data_gas_price(&self) -> Result<u128, Error> {
        Ok(
            ((TryInto::<u64>::try_into(self.l1_data_gas_price).map_err(|_| Error::Internal("Fee out of range".to_string()))? as f64) * self.gas_price_estimate_multiplier)
                as u128,
        )
    }
}
