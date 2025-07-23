use starknet::core::types::Felt;

/// Represents the gas price at the given block in wei
#[derive(Default, Debug, Clone, Copy)]
pub struct BlockGasPrice {
    pub computation: Felt,
    pub storage: Felt,
}
