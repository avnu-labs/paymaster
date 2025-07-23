use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::CalldataBuilder;
use paymaster_starknet::ChainID;
use starknet::core::types::{Call, Felt};
use starknet::macros::selector;

pub fn an_eth_transfer(to: Felt, amount: Felt, chain_id: &ChainID) -> Call {
    Call {
        to: Token::eth(chain_id).address,
        selector: selector!("transfer"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}

pub fn an_eth_approve(to: Felt, amount: Felt, chain_id: &ChainID) -> Call {
    Call {
        to: Token::eth(chain_id).address,
        selector: selector!("approve"),
        calldata: CalldataBuilder::new().encode(&to).encode(&amount).encode(&Felt::ZERO).build(),
    }
}
