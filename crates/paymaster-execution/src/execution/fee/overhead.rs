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

    /// Additional cost for Shhh Ed25519 wallet — Garaga v1.0.1 on-chain EdDSA verification.
    /// Enables Solana (Phantom) users to operate Starknet smart accounts with their existing wallet.
    /// Breakdown: Garaga MSM + EdDSA (~33M) + bytes_to_hex_ascii loop (~5M) +
    /// OE byte encoding + Poseidon (~3M) + Serde deserialization of ~100 felt252 hints (~5M) +
    /// message byte comparison loop 372 iterations (~2M) + storage reads/writes (~2M).
    /// See: https://github.com/haycarlitos/shhh-wallet-cairo
    fn shhh_ed25519() -> Self {
        Self {
            l1_gas: Felt::ZERO,
            l1_data_gas: Felt::ZERO,
            l2_gas: felt!("0x04c4b400"), // ~80M L2 gas
        }
    }

    /// Additional cost for Shhh EVM wallet — native secp256k1 verify_eth_signature syscall.
    /// Enables MetaMask/EVM users to operate Starknet smart accounts.
    /// Breakdown: keccak256 syscall for EIP-191 hash (~5M) + verify_eth_signature/ecrecover (~8M) +
    /// OE byte encoding + Poseidon (~3M) + ByteArray construction loops (~2M) +
    /// storage reads/writes (~2M).
    fn shhh_evm() -> Self {
        Self {
            l1_gas: Felt::ZERO,
            l1_data_gas: Felt::ZERO,
            l2_gas: felt!("0x01312D00"), // ~20M L2 gas
        }
    }

    /// Additional cost for SNIP-163 session accounts (Chipi Pay reference implementation).
    /// Session key validation involves dual-hash SNIP-12 computation (worst case: two full
    /// Poseidon hashes + two ECDSA verifications), session data storage reads,
    /// entrypoint whitelist iteration, call consumption writes, and spending policy checks.
    /// See: https://github.com/starknet-io/SNIPs/pull/163
    /// See: https://github.com/chipi-pay/sessions-smart-contract
    fn chipi_sessions() -> Self {
        Self {
            l1_gas: Felt::ZERO,
            l1_data_gas: Felt::ZERO,
            l2_gas: felt!("0x01312D00"), // ~20M L2 gas
        }
    }

    /// Returns the overhead approximation given the [`user`] address.
    ///
    /// Detection order: Shhh Ed25519 → Shhh EVM → SNIP-163 Sessions → Braavos → none.
    /// Each check calls a unique entrypoint on the account; results are cached upstream.
    pub async fn fetch(client: &Client, user: ContractAddress) -> Result<Self, Error> {
        // 1. Shhh Ed25519 wallet: get_owner() returns u256 (exactly 2 felts)
        let shhh_call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("get_owner"),
            calldata: vec![],
        };
        match client.call(&shhh_call).await {
            Ok(response) if response.len() == 2 => return Ok(Self::shhh_ed25519()),
            Ok(_) => {},
            Err(e) => {
                tracing::debug!(user = %user.to_fixed_hex_string(), error = %e, "get_owner failed (expected for non-Shhh Ed25519)");
            },
        }

        // 2. Shhh EVM wallet: get_eth_address() returns EthAddress (exactly 1 felt)
        let evm_call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("get_eth_address"),
            calldata: vec![],
        };
        match client.call(&evm_call).await {
            Ok(response) if response.len() == 1 => return Ok(Self::shhh_evm()),
            Ok(_) => {},
            Err(e) => {
                tracing::debug!(user = %user.to_fixed_hex_string(), error = %e, "get_eth_address failed (expected for non-Shhh EVM)");
            },
        }

        // 3. SNIP-163 session accounts: supports_interface(SESSION_KEY_MANAGER_ID)
        //    Interface ID from SNIP-163 Part G: XOR of starknetKeccak of ISessionKeyManager selectors
        let session_call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("supports_interface"),
            calldata: vec![felt!("0x037ab4f01106526662a612eaa2926df2aa314c4144b964f183805880bbcfa55d")],
        };
        match client.call(&session_call).await {
            Ok(response) if response.first() == Some(&Felt::ONE) => return Ok(Self::chipi_sessions()),
            Ok(_) => {},
            Err(e) => {
                tracing::debug!(user = %user.to_fixed_hex_string(), error = %e, "supports_interface(SESSION_KEY_MANAGER) failed");
            },
        }

        // 4. Braavos MFA (existing)
        let braavos_call = FunctionCall {
            contract_address: user,
            entry_point_selector: selector!("get_signers"),
            calldata: vec![],
        };
        match client.call(&braavos_call).await {
            Ok(response) if response.len() > 4 => Ok(Self::braavos()),
            Ok(_) => Ok(Self::none()),
            Err(e) => {
                tracing::debug!(user = %user.to_fixed_hex_string(), error = %e, "get_signers failed (expected for non-Braavos)");
                Ok(Self::none())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_overhead_is_zero() {
        let overhead = ValidationGasOverhead::none();
        assert_eq!(overhead.l1_gas, Felt::ZERO);
        assert_eq!(overhead.l1_data_gas, Felt::ZERO);
        assert_eq!(overhead.l2_gas, Felt::ZERO);
    }

    #[test]
    fn test_braavos_overhead() {
        let overhead = ValidationGasOverhead::braavos();
        assert_eq!(overhead.l2_gas, felt!("0x02c7ab80"));
        assert_eq!(overhead.l1_gas, Felt::ZERO);
    }

    #[test]
    fn test_shhh_ed25519_overhead() {
        let overhead = ValidationGasOverhead::shhh_ed25519();
        assert_eq!(overhead.l2_gas, felt!("0x04c4b400")); // ~80M
        assert_eq!(overhead.l1_gas, Felt::ZERO);
        assert_eq!(overhead.l1_data_gas, Felt::ZERO);
    }

    #[test]
    fn test_shhh_evm_overhead() {
        let overhead = ValidationGasOverhead::shhh_evm();
        assert_eq!(overhead.l2_gas, felt!("0x01312D00")); // ~20M
        assert_eq!(overhead.l1_gas, Felt::ZERO);
    }

    #[test]
    fn test_chipi_sessions_overhead() {
        let overhead = ValidationGasOverhead::chipi_sessions();
        assert_eq!(overhead.l2_gas, felt!("0x01312D00")); // ~20M
        assert_eq!(overhead.l1_gas, Felt::ZERO);
    }

    #[test]
    fn test_overhead_ordering() {
        // Ed25519 has highest overhead (Garaga MSM), all others are lower
        let ed25519 = ValidationGasOverhead::shhh_ed25519();
        let evm = ValidationGasOverhead::shhh_evm();
        let sessions = ValidationGasOverhead::chipi_sessions();
        let braavos = ValidationGasOverhead::braavos();
        let none = ValidationGasOverhead::none();
        assert!(ed25519.l2_gas > braavos.l2_gas);
        assert!(braavos.l2_gas > sessions.l2_gas);
        assert!(sessions.l2_gas >= evm.l2_gas);
        assert!(evm.l2_gas > none.l2_gas);
    }
}
