use starknet::core::types::{FeeEstimate, Felt, PriceUnit};

use crate::Error;

#[derive(Debug, Clone)]
pub struct TransactionGasEstimate {
    pub overall_fee: u128,
    pub unit: PriceUnit,
    l1_gas_consumed: u64,
    l1_gas_price: u128,
    l2_gas_consumed: u64,
    l2_gas_price: u128,
    l1_data_gas_consumed: u64,
    l1_data_gas_price: u128,
    gas_estimate_multiplier: f64,
    gas_price_estimate_multiplier: f64,
}

impl From<FeeEstimate> for TransactionGasEstimate {
    fn from(value: FeeEstimate) -> Self {
        Self::new(value)
    }
}

impl TransactionGasEstimate {
    pub fn new(estimate: FeeEstimate) -> Self {
        Self {
            overall_fee: estimate.overall_fee,
            l1_gas_price: estimate.l1_gas_price,
            l2_gas_price: estimate.l2_gas_price,
            l1_data_gas_price: estimate.l1_data_gas_price,
            l1_gas_consumed: estimate.l1_gas_consumed,
            l2_gas_consumed: estimate.l2_gas_consumed,
            l1_data_gas_consumed: estimate.l1_data_gas_consumed,
            unit: estimate.unit,
            gas_estimate_multiplier: 1.5,
            gas_price_estimate_multiplier: 1.5,
        }
    }

    pub fn update_overall_fee(self, overall_fee: Felt) -> Self {
        // Calculate the L2 gas consumed based on the overall fee and the L1 gas and data gas consumed
        // The new overall fee includes validation headers. The validation overhead only applies to l2_gas_consumed
        let l2_gas_consumed = if self.l2_gas_consumed != 0 {
            ((felt_to_u128(&overall_fee) - (self.l1_gas_consumed as u128 * self.l1_gas_price + self.l1_data_gas_consumed as u128 * self.l1_data_gas_price))
                / self.l2_gas_price) as u64
        } else {
            self.l2_gas_consumed
        };
        Self {
            overall_fee: felt_to_u128(&overall_fee),
            l1_gas_price: self.l1_gas_price,
            l2_gas_price: self.l2_gas_price,
            l1_data_gas_price: self.l1_data_gas_price,
            l1_gas_consumed: self.l1_gas_consumed,
            l2_gas_consumed,
            l1_data_gas_consumed: self.l1_data_gas_consumed,
            unit: self.unit,
            gas_estimate_multiplier: self.gas_estimate_multiplier,
            gas_price_estimate_multiplier: self.gas_price_estimate_multiplier,
        }
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

fn felt_to_u128(felt: &Felt) -> u128 {
    let bytes = felt.to_bytes_le();
    let slice: [u8; 16] = bytes[..16].try_into().expect("Felt should have at least 16 bytes");
    u128::from_le_bytes(slice)
}
