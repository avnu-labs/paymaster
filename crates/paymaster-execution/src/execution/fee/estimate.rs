use starknet::core::types::Felt;

#[derive(Debug)]
pub struct FeeEstimate {
    pub gas_token_price_in_strk: Felt,
    pub estimated_fee_in_strk: Felt,
    pub estimated_fee_in_gas_token: Felt,
    pub suggested_max_fee_in_strk: Felt,
    pub suggested_max_fee_in_gas_token: Felt,
}

impl FeeEstimate {
    /// Fee estimate returned for sponsored transactions. The user does not pay any fee and the
    /// transaction is simulated again at execution time, so no estimate is computed at build time.
    pub fn sponsored() -> Self {
        Self {
            gas_token_price_in_strk: Felt::ZERO,
            estimated_fee_in_strk: Felt::ZERO,
            estimated_fee_in_gas_token: Felt::ZERO,
            suggested_max_fee_in_strk: Felt::ZERO,
            suggested_max_fee_in_gas_token: Felt::ZERO,
        }
    }
}
