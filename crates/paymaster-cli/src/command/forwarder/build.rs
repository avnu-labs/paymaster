use paymaster_starknet::constants::ClassHash;
use paymaster_starknet::transaction::{CalldataBuilder, Calls};
use paymaster_starknet::ContractAddress;
use starknet::core::types::Felt;

use crate::core::starknet::transaction::deploy::DeployTransaction;
use crate::core::Error;

pub struct ForwarderDeployment {
    pub address: ContractAddress,
    pub calls: Calls,
}

impl ForwarderDeployment {
    pub async fn build_transaction(owner: Felt, gas_tank_address: Felt) -> Result<DeployTransaction, Error> {
        let paymaster_deploy = DeployTransaction {
            class_hash: ClassHash::FORWARDER,
            calldata: CalldataBuilder::new().encode(&owner).encode(&gas_tank_address).build(),
        };

        Ok(paymaster_deploy)
    }

    pub async fn build(owner: Felt, gas_tank_address: Felt) -> Result<Self, Error> {
        let paymaster_deploy = Self::build_transaction(owner, gas_tank_address).await?;
        let (contract, calls) = paymaster_deploy.build().await?;

        Ok(Self { address: contract, calls })
    }
}
