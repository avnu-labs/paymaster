use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::CalldataBuilder;
use starknet::core::types::{Call, Felt};
use starknet::macros::selector;

pub fn an_eth_transfer(to: Felt, amount: Felt) -> Call {
    Call {
        to: Token::ETH_ADDRESS,
        selector: selector!("transfer"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}

pub fn an_eth_approve(to: Felt, amount: Felt) -> Call {
    Call {
        to: Token::ETH_ADDRESS,
        selector: selector!("approve"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}
