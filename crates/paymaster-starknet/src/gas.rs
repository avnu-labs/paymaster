use starknet::core::types::Felt;

/// Represents the gas price at the given block in wei
#[derive(Default, Debug, Clone, Copy)]
pub struct BlockGasPrice {
    pub l1_gas_price: Felt,
    pub l1_data_gas_price: Felt,
    pub l2_gas_price: Felt,
}
