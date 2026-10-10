mod overhead;
pub use overhead::ValidationGasOverhead;

mod estimate;
pub use estimate::FeeEstimate;

use starknet::core::types::Felt;

/// Action describing a fee payment the user must include in their private transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeeAction {
    /// The user must add a withdrawal of `amount` of `token` to `recipient` in their proof
    Withdraw { recipient: Felt, token: Felt, amount: Felt },
    /// Nothing to add to the proof: the sponsor pays the pool fee
    None,
}
