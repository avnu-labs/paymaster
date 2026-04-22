mod overhead;
pub use overhead::ValidationGasOverhead;

mod estimate;
pub use estimate::FeeEstimate;

use starknet::core::types::Felt;

/// Action describing a fee payment the user must include in their private transaction.
#[derive(Debug, Clone)]
pub enum FeeAction {
    Withdraw { recipient: Felt, token: Felt, amount: Felt },
}
