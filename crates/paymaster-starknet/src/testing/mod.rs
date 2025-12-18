pub mod transaction;

use std::ops::Deref;
use std::time::Duration;

use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::macros::felt;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::time;

use crate::constants::Token;
use crate::transaction::TokenTransfer;
use crate::{ChainID, Client, Configuration, StarknetAccountConfiguration};

pub type StarknetContainer = ContainerAsync<GenericImage>;

pub struct TestEnvironment {
    configuration: Configuration,

    pub client: Client,

    #[allow(dead_code)]
    pub container: StarknetContainer,
}

impl Deref for TestEnvironment {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl TestEnvironment {
    pub const ACCOUNT_1: StarknetAccountConfiguration = StarknetAccountConfiguration {
        address: felt!("0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691"),
        private_key: felt!("0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9"),
    };
    pub const ACCOUNT_2: StarknetAccountConfiguration = StarknetAccountConfiguration {
        address: felt!("0x078662e7352d062084b0010068b99288486c2d8b914f6e2a55ce945f8792c8b1"),
        private_key: felt!("0x000000000000000000000000000000000e1406455b7d66b1690803be066cbe5e"),
    };
    pub const ACCOUNT_3: StarknetAccountConfiguration = StarknetAccountConfiguration {
        address: felt!("0x049dfb8ce986e21d354ac93ea65e6a11f639c1934ea253e5ff14ca62eca0f38e"),
        private_key: felt!("0x00000000000000000000000000000000a20a02f0ac53692d144b20cb371a60d7"),
    };
    pub const ACCOUNT_ARGENT_1: StarknetAccountConfiguration = StarknetAccountConfiguration {
        address: felt!("0x021482d2d427705459ea21f1ed22a769ec6358d7024c17eddf3bbfdf083b8b80"),
        private_key: felt!("0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9"),
    };
    const CHAIN_ID: ChainID = ChainID::Sepolia;
    pub const ETH: Felt = Token::ETH_ADDRESS;
    pub const FORWARDER: Felt = felt!("0x04c4b4e84d06f13690dfbd43fae5cc0e7f122756e50df2236b41ca6afff775e6");
    pub const GAS_TANK: StarknetAccountConfiguration = StarknetAccountConfiguration {
        address: felt!("0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691"),
        private_key: felt!("0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9"),
    };
    pub const NETWORK: &'static str = "SN_SEPOLIA";
    pub const RELAYER_1: Felt = felt!("0x016f3f34c417aa41782bc641bfcd08764738344034ee760b0d00bea3cdb9b258");
    pub const RELAYER_2: Felt = felt!("0x0365133c36063dabe51611dd8a83ca4a31944ea87bd7ef3da576b754be098dc1");
    pub const RELAYER_3: Felt = felt!("0x055c5d84d644301e4d2375c93868484c94a76bd68a565620bda3473efb4cf9a0");
    pub const RELAYER_PRIVATE_KEY: Felt = felt!("0x0000000000000000000000000000000071d7bb07b9a64f6f78ac4c816aff4da9");
    pub const STRK: Felt = Token::STRK_ADDRESS;
    pub const USDC: Felt = Token::usdc(&Self::CHAIN_ID).address;

    pub async fn new() -> Self {
        let container = Self::start_starknet().await;
        let endpoint = format!("http://localhost:{}", container.get_host_port_ipv4(5050).await.unwrap());

        let configuration = Configuration {
            chain_id: Self::CHAIN_ID,
            timeout: 10,
            endpoint,
            fallbacks: vec![],
        };

        Self {
            client: Client::new(&configuration),
            container,
            configuration,
        }
    }

    async fn start_starknet() -> StarknetContainer {
        GenericImage::new("avnulabs/paymaster-ci-starknet", "0.5.0")
            .with_exposed_port(5050.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Starknet Devnet"))
            .start()
            .await
            .unwrap()
    }

    pub fn configuration(&self) -> Configuration {
        self.configuration.clone()
    }

    pub async fn transfer_token<A>(&self, account: &A, transfer: &TokenTransfer)
    where
        A: Account + ConnectedAccount,
        A: Sync,
    {
        let transfer = transfer.to_call();

        let nonce = account.get_nonce().await.unwrap();

        let tx_hash = account
            .execute_v3(vec![transfer])
            .nonce(nonce)
            .send()
            .await
            .unwrap()
            .transaction_hash;

        while self.client.get_transaction_receipt(tx_hash).await.is_err() {
            time::sleep(Duration::from_secs(1)).await;
        }
    }
}
