use starknet::core::types::{FeeEstimate, Felt};

use crate::Error;

/// Default upper bound (in L2 gas units) the paymaster will ever declare as `max_l2_gas_amount` on a
/// transaction. Set to exactly the Starknet sequencer gateway `max_l2_gas_amount` on mainnet: the gateway
/// rejects, during stateless validation (before broadcast), any transaction whose declared L2 gas amount
/// is *strictly greater* (`>`) than this value, so declaring exactly it is accepted. Capping the padded
/// estimate here keeps a heavy but legitimate transaction from being rejected purely because of the
/// gas-bound padding multiplier. Configurable per network since the sequencer value may change over time.
pub const DEFAULT_MAX_L2_GAS_AMOUNT: u64 = 1_200_000_000;

/// Safety margin applied to the *actual* estimated L2 gas when deciding whether a transaction can safely
/// fit under the cap. If the estimate times this margin already exceeds the cap, capping the declared
/// bound would leave no headroom and the transaction would almost certainly revert out-of-gas, so we
/// reject it early instead of wasting relayer gas.
const L2_GAS_CAP_SAFETY_MULTIPLIER: f64 = 1.1;

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
    max_l2_gas_amount: u64,
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
            max_l2_gas_amount: DEFAULT_MAX_L2_GAS_AMOUNT,
        }
    }

    /// Override the maximum L2 gas amount the paymaster is willing to declare as a bound. This mirrors
    /// the Starknet sequencer gateway `max_l2_gas_amount` (minus a safety margin) and is configurable
    /// per network.
    pub fn with_max_l2_gas_amount(mut self, max_l2_gas_amount: u64) -> Self {
        self.max_l2_gas_amount = max_l2_gas_amount;
        self
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
            max_l2_gas_amount: self.max_l2_gas_amount,
        }
    }

    pub fn tip(&self) -> u64 {
        self.tip
    }

    /// Ensure the transaction can be declared with enough L2 gas headroom to fit under the sequencer
    /// cap. Returns an error when even the bare estimate (plus a small safety margin) exceeds the cap,
    /// since capping the declared bound in that case would lead to an out-of-gas revert and waste
    /// relayer gas. Capping happens in [`Self::l2_gas_consumed`]; this guards the case capping cannot
    /// save.
    pub fn check_l2_gas_within_cap(&self) -> Result<(), Error> {
        let required = (self.l2_gas_consumed as f64 * L2_GAS_CAP_SAFETY_MULTIPLIER) as u64;
        if required > self.max_l2_gas_amount {
            return Err(Error::MaxL2GasAmountExceeded {
                required,
                max: self.max_l2_gas_amount,
            });
        }

        Ok(())
    }

    pub fn l1_gas_consumed(&self) -> u64 {
        ((self.l1_gas_consumed as f64) * self.gas_estimate_multiplier) as u64
    }

    pub fn l2_gas_consumed(&self) -> u64 {
        // Cap the declared L2 gas bound at `max_l2_gas_amount`: the padded estimate (raw * 1.5) can
        // exceed the sequencer gateway limit and get the transaction rejected before broadcast, even
        // though it only consumes a fraction of it. L1 gas / L1 data gas keep their full padding.
        ((self.l2_gas_consumed as f64 * self.gas_estimate_multiplier) as u64).min(self.max_l2_gas_amount)
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

#[cfg(test)]
mod tests {
    use starknet::core::types::FeeEstimate;

    use super::*;

    fn estimate_with_l2_gas(l2_gas_consumed: u64) -> TransactionGasEstimate {
        TransactionGasEstimate::new(
            FeeEstimate {
                l1_gas_consumed: 0,
                l1_gas_price: 1,
                l2_gas_consumed,
                l2_gas_price: 1,
                l1_data_gas_consumed: 0,
                l1_data_gas_price: 1,
                overall_fee: l2_gas_consumed as u128,
            },
            0,
        )
    }

    #[test]
    fn pads_l2_gas_by_multiplier_when_below_cap() {
        // 100M raw -> 150M declared bound, well under the cap; nothing is clamped.
        let estimate = estimate_with_l2_gas(100_000_000);
        assert_eq!(estimate.l2_gas_consumed(), 150_000_000);
        assert!(estimate.check_l2_gas_within_cap().is_ok());
    }

    #[test]
    fn caps_l2_gas_bound_when_padding_exceeds_cap() {
        // 900M raw -> 1.35B padded -> clamped to the 1.15B cap. The tx still fits (900M * 1.1 <= cap).
        let estimate = estimate_with_l2_gas(900_000_000).with_max_l2_gas_amount(DEFAULT_MAX_L2_GAS_AMOUNT);
        assert_eq!(estimate.l2_gas_consumed(), DEFAULT_MAX_L2_GAS_AMOUNT);
        assert!(estimate.check_l2_gas_within_cap().is_ok());
    }

    #[test]
    fn rejects_early_when_estimate_does_not_fit_under_cap() {
        // 1.1B raw -> 1.1B * 1.1 = 1.21B > 1.15B cap: capping cannot give headroom, reject before broadcast.
        let estimate = estimate_with_l2_gas(1_100_000_000).with_max_l2_gas_amount(DEFAULT_MAX_L2_GAS_AMOUNT);
        assert!(matches!(estimate.check_l2_gas_within_cap(), Err(Error::MaxL2GasAmountExceeded { .. })));
    }

    #[test]
    fn applies_custom_cap_override() {
        // The cap is configurable: a lower override clamps the declared bound accordingly.
        let estimate = estimate_with_l2_gas(100_000_000).with_max_l2_gas_amount(120_000_000);
        assert_eq!(estimate.l2_gas_consumed(), 120_000_000);
    }

    #[test]
    fn cap_is_preserved_through_update_overall_fee() {
        // The cap field must survive the re-derivation done at execute time.
        let estimate = estimate_with_l2_gas(900_000_000)
            .with_max_l2_gas_amount(DEFAULT_MAX_L2_GAS_AMOUNT)
            .update_overall_fee(Felt::from(900_000_000u64));
        assert_eq!(estimate.l2_gas_consumed(), DEFAULT_MAX_L2_GAS_AMOUNT);
    }
}
