use std::ops::Mul;

use paymaster_starknet::{BlockGasPrice, ContractAddress};
use starknet::core::types::{Felt, FunctionCall};
use starknet::macros::{felt, selector};

use crate::starknet::Client;
use crate::Error;

/// Computation and storage overhead induced by the account type. This is an approximation
/// added on top of the original estimate.
#[derive(Debug, Default, Clone, Copy)]
pub struct ValidationGasOverhead {
    pub computation: Felt,
    pub storage: Felt,
}

impl Mul<ValidationGasOverhead> for BlockGasPrice {
    type Output = Felt;

    fn mul(self, rhs: ValidationGasOverhead) -> Self::Output {
        self.storage * rhs.storage + self.computation * rhs.computation
    }
}

impl ValidationGasOverhead {
    /// No additional gos
    fn none() -> Self {
        Self::default()
    }

    /// Additional cost induced by Braavos account
    fn braavos() -> Self {
        Self {
            computation: felt!("0x0460"),
            storage: Felt::ZERO,
        }
    }

    /// Returns the overhead approximation given the [`user`] address
    pub async fn fetch(client: &Client, user: ContractAddress) -> Result<Self, Error> {
        let call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("get_signers"), // This endpoint is specific to Braavos
            calldata: vec![],
        };

        match client.call(&call).await {
            Ok(response) if response.len() > 4 => Ok(Self::braavos()),
            Ok(_) | Err(paymaster_starknet::Error::ContractNotFound) | Err(paymaster_starknet::Error::Contract(_)) => Ok(Self::none()),
            Err(_) => Ok(Self::none()),
        }
    }
}
