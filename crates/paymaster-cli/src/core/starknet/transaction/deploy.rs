use paymaster_starknet::constants::Contract;
use paymaster_starknet::transaction::{CalldataBuilder, Calls};
use paymaster_starknet::{Client, StarknetAccount};
use starknet::accounts::{AccountFactory, ConnectedAccount};
use starknet::core::types::{Call, Felt};
use starknet::core::utils::{get_udc_deployed_address, UdcUniqueness};
use starknet::macros::selector;
use uuid::Uuid;

use crate::core::Error;

pub struct DeployTransaction {
    pub class_hash: Felt,
    pub calldata: Vec<Felt>,
}

impl DeployTransaction {
    pub async fn build(self) -> Result<(Felt, Calls), Error> {
        let salt = Felt::from(Uuid::new_v4().as_u128());
        let contract = get_udc_deployed_address(
            salt,
            self.class_hash,
            &UdcUniqueness::NotUnique,
            &CalldataBuilder::new().encode(&self.calldata).build()[1..],
        );

        let calls = Calls::new(vec![self.as_call(salt)]);

        Ok((contract, calls))
    }

    pub async fn execute(self, account: &StarknetAccount) -> Result<(Felt, Felt), Error> {
        let (contract, calls) = self.build().await?;
        let estimated_calls = calls.estimate(account).await.unwrap();

        let nonce = account.get_nonce().await.unwrap();
        let result = estimated_calls.execute(account, nonce).await.unwrap();

        Ok((contract, result.transaction_hash))
    }

    fn as_call(&self, salt: Felt) -> Call {
        Call {
            to: Contract::UDC, // UDC
            selector: selector!("deployContract"),
            calldata: CalldataBuilder::new()
                .encode(&self.class_hash)
                .encode(&salt)
                .encode(&Felt::ZERO)
                .encode(&self.calldata)
                .build(),
        }
    }
}

pub struct DeployArgentAccount {
    pub private_key: Felt,
    pub address: Felt,
    pub class_hash: Felt,
    pub salt: Felt,
    pub calldata: Vec<Felt>,
}

impl DeployArgentAccount {
    pub async fn initialize(starknet: &Client, private_key: Felt) -> Self {
        let account = starknet.initialize_argent_account(private_key).await;
        let salt = Felt::from(Uuid::new_v4().as_u128());
        let deploy = account.deploy_v3(salt);

        Self {
            private_key,
            address: deploy.address(),
            class_hash: account.class_hash(),
            salt,
            calldata: account.calldata(),
        }
    }

    pub async fn deploy(self, account: &StarknetAccount) -> Result<Felt, Error> {
        let calls = Calls::new(vec![self.as_call()]);
        let nonce = account.get_nonce().await.unwrap();
        calls.execute(account, nonce).await.unwrap();
        Ok(self.address)
    }

    pub fn as_call(&self) -> Call {
        DeployTransaction {
            class_hash: self.class_hash,
            calldata: self.calldata.clone(),
        }
        .as_call(self.salt)
    }
}
