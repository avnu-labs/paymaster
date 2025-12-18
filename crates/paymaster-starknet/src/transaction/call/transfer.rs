use std::ops::Deref;

use starknet::core::types::{Call, Felt};
use starknet::macros::selector;

use crate::constants::Token;
use crate::transaction::call::calldata::CalldataBuilder;

pub struct StrkTransfer(TokenTransfer);

impl Deref for StrkTransfer {
    type Target = TokenTransfer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StrkTransfer {
    pub fn new(sender: Felt, amount: Felt) -> Self {
        Self(TokenTransfer::new(Token::STRK_ADDRESS, sender, amount))
    }

    pub fn to_call(&self) -> Call {
        self.0.to_call()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TokenTransfer {
    recipient: Felt,
    token: Felt,
    amount: Felt,
}

impl TokenTransfer {
    pub fn new(token: Felt, recipient: Felt, amount: Felt) -> Self {
        Self { token, recipient, amount }
    }

    pub fn recipient(&self) -> Felt {
        self.recipient
    }

    pub fn amount(&self) -> Felt {
        self.amount
    }

    pub fn token(&self) -> Felt {
        self.token
    }

    pub fn to_call(&self) -> Call {
        Call {
            to: self.token,
            selector: selector!("transfer"),
            calldata: CalldataBuilder::new()
                .encode(&self.recipient)
                .encode(&self.amount)
                .encode(&Felt::ZERO)
                .build(),
        }
    }
}
