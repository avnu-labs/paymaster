use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::Calls;
use paymaster_starknet::{Client, ContractAddress};
use starknet::core::types::Felt;

use crate::core::starknet::transaction::deploy::DeployArgentAccount;
use crate::core::starknet::transaction::transfer::Transfer;
use crate::core::Error;

pub struct GasTankDeployment {
    pub address: ContractAddress,
    pub calls: Calls,
}

impl GasTankDeployment {
    pub async fn build(starknet: &Client, private_key: Felt, fund: Felt) -> Result<Self, Error> {
        let gas_tank_deployment = DeployArgentAccount::initialize(&starknet, private_key).await;
        // Fund the gas tank with the amount of STRK specified in the parameters (1 STRK will be used as reserve)
        let transfer_call = Transfer {
            token: Token::strk(starknet.chain_id()).address,
            recipient: gas_tank_deployment.address,
            amount: fund,
        }
        .as_call();

        // build multicall
        let mut calls = Calls::empty();
        calls.push(gas_tank_deployment.as_call());
        calls.push(transfer_call);

        Ok(Self {
            address: gas_tank_deployment.address,
            calls,
        })
    }
}
