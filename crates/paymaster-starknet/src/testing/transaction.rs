use starknet::core::types::{Call, Felt};
use starknet::macros::selector;

use crate::constants::Token;
use crate::transaction::CalldataBuilder;

pub fn an_eth_transfer(to: Felt, amount: Felt) -> Call {
    Call {
        to: Token::ETH_ADDRESS,
        selector: selector!("transfer"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}
