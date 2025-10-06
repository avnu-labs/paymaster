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
    pub l1_gas: Felt,
    pub l1_data_gas: Felt,
    pub l2_gas: Felt,
}

impl Mul<ValidationGasOverhead> for BlockGasPrice {
    type Output = Felt;

    fn mul(self, rhs: ValidationGasOverhead) -> Self::Output {
        self.l1_gas_price * rhs.l1_gas + self.l1_data_gas_price * rhs.l1_data_gas + self.l2_gas_price * rhs.l2_gas
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
            l1_gas: Felt::ZERO,
            l1_data_gas: Felt::ZERO,
            l2_gas: felt!("0x02c7ab80"),
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
