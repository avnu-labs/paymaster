use starknet::core::types::Felt;

#[derive(Debug)]
pub struct FeeEstimate {
    pub gas_token_price_in_strk: Felt,
    pub estimated_fee_in_strk: Felt,
    pub estimated_fee_in_gas_token: Felt,
    pub suggested_max_fee_in_strk: Felt,
    pub suggested_max_fee_in_gas_token: Felt,
}
