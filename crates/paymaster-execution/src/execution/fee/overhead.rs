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
    /// No additional gas
    pub fn none() -> Self {
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

    fn from_get_signers_response(response: &[Felt]) -> Self {
        if response.len() > 4 {
            Self::braavos()
        } else {
            Self::none()
        }
    }

    /// Returns the overhead approximation given the [`user`] address. An `Err` means the
    /// detection is not authoritative (contract not deployed yet, transient RPC error)
    /// and the caller should not cache the result.
    pub async fn fetch(client: &Client, user: ContractAddress) -> Result<Self, Error> {
        let call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("get_signers"), // This endpoint is specific to Braavos
            calldata: vec![],
        };

        match client.call(&call).await {
            Ok(response) => Ok(Self::from_get_signers_response(&response)),
            Err(paymaster_starknet::Error::Execution(_)) | Err(paymaster_starknet::Error::Contract(_)) => Ok(Self::none()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_get_signers_response_classic_braavos_4_felts_returns_none() {
        let response = vec![
            felt!("0x1"),
            felt!("0x14ffa4f8f8770236d284eee22b52120102e22af8c9ca1a045084be9b1e7d1b9"),
            Felt::ZERO,
            Felt::ZERO,
        ];
        let overhead = ValidationGasOverhead::from_get_signers_response(&response);
        assert_eq!(overhead.l2_gas, Felt::ZERO);
    }

    #[test]
    fn from_get_signers_response_braavos_passkey_5_felts_returns_braavos() {
        let response = vec![
            felt!("0x1"),
            felt!("0x4221c76ba6e0e6b0c9b51bf74c9bd84ca40c33049443f7b3700c982f3348163"),
            felt!("0x1"),
            felt!("0x4939ff2d144c912b3bd7906928101c1a1c13a342911216dce9192d2114ae71"),
            Felt::ZERO,
        ];
        let overhead = ValidationGasOverhead::from_get_signers_response(&response);
        assert_eq!(overhead.l2_gas, felt!("0x02c7ab80"));
    }

    #[test]
    fn from_get_signers_response_empty_returns_none() {
        let overhead = ValidationGasOverhead::from_get_signers_response(&[]);
        assert_eq!(overhead.l2_gas, Felt::ZERO);
    }
}
