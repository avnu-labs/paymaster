use starknet::core::types::{Call, Felt};
use starknet::macros::selector;

use crate::constants::Token;
use crate::transaction::CalldataBuilder;
use crate::ChainID;

pub fn an_eth_transfer(to: Felt, amount: Felt, chain_id: &ChainID) -> Call {
    Call {
        to: Token::eth(chain_id).address,
        selector: selector!("transfer"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}
