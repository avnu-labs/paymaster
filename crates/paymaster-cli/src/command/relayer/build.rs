use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::{CalldataBuilder, Calls};
use paymaster_starknet::{Client, ContractAddress};
use starknet::core::types::Felt;
use starknet::macros::selector;

use crate::core::starknet::transaction::deploy::DeployArgentAccount;
use crate::core::starknet::transaction::invoke::InvokeTransaction;
use crate::core::starknet::transaction::transfer::Transfer;
use crate::core::Error;

pub struct RelayerDeployment {
    pub addresses: Vec<ContractAddress>,
    pub calls: Calls,
}

pub struct SingleRelayerDeployment {
    pub address: ContractAddress,
    pub calls: Calls,
}

impl RelayerDeployment {
    pub async fn build_one(starknet: &Client, forwarder: Felt, private_key: Felt, fund: Felt) -> Result<SingleRelayerDeployment, Error> {
        let deploy_relayer = DeployArgentAccount::initialize(starknet, private_key).await;

        let whitelist = InvokeTransaction {
            to: forwarder,
            selector: selector!("set_whitelisted_address"),
            calldata: CalldataBuilder::new()
                .encode(&deploy_relayer.address)
                .encode(&Felt::ONE)
                .build(),
        };

        let fund_transfer = Transfer {
            recipient: deploy_relayer.address,
            token: Token::strk(starknet.chain_id()).address,
            amount: fund,
        };

        let mut calls = Calls::empty();
        calls.push(deploy_relayer.as_call());
        calls.push(whitelist.as_call());
        if fund != Felt::ZERO {
            calls.push(fund_transfer.as_call());
        }

        Ok(SingleRelayerDeployment {
            address: deploy_relayer.address,
            calls,
        })
    }

    pub async fn build_many(starknet: &Client, forwarder: Felt, private_key: Felt, count: usize, fund: Felt) -> Result<Self, Error> {
        let mut deployment = vec![];
        for _ in 0..count {
            deployment.push(RelayerDeployment::build_one(&starknet, forwarder, private_key, fund).await?);
        }

        let calls = deployment.iter().fold(Calls::empty(), |mut calls, x| {
            calls.merge(&x.calls);
            calls
        });

        let addresses = deployment.into_iter().map(|x| x.address).collect();
        Ok(Self { addresses, calls })
    }
}
