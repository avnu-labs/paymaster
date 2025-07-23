use std::collections::HashMap;
use std::time::Duration;

use paymaster_common::cache::ExpirableCache;
use paymaster_common::service::messaging::Messages;
use paymaster_starknet::{Client, StarknetAccountConfiguration};
use starknet::core::types::Felt;

use crate::relayer::Relayer;
use crate::{Error, Message, RelayerConfiguration, RelayersConfiguration};

#[derive(Clone)]
pub struct Relayers {
    relayers: HashMap<Felt, Relayer>,
    // Map of relayer address to its balance
    balances: ExpirableCache<Felt, Felt>,
}

impl Relayers {
    pub fn new(starknet: &Client, messages: Messages<Message>, configuration: &RelayersConfiguration) -> Self {
        let mut relayers = HashMap::new();
        let num_relayers = configuration.addresses.len().try_into().unwrap();
        let balances = ExpirableCache::new(num_relayers);
        for address in &configuration.addresses {
            relayers.insert(
                *address,
                Relayer::new(
                    starknet,
                    messages.clone(),
                    balances.clone(),
                    &RelayerConfiguration {
                        account: StarknetAccountConfiguration {
                            address: *address,
                            private_key: configuration.private_key,
                        },
                    },
                ),
            );
        }

        Self { relayers, balances }
    }

    pub fn acquire_relayer(&self, relayer: &Felt) -> Result<Relayer, Error> {
        self.relayers.get(relayer).cloned().ok_or(Error::InvalidRelayer)
    }

    pub async fn get_relayer_balance(&self, relayer: &Felt) -> Option<Felt> {
        self.balances.get_if_not_stale(relayer)
    }

    pub async fn set_relayer_balance(&self, relayer: Felt, balance: Felt) {
        self.balances.insert(relayer, balance, Duration::from_secs(60));
    }

    // Set balance with custom validity
    pub async fn set_relayer_balance_with_validity(&self, relayer: Felt, balance: Felt, validity: Duration) {
        self.balances.insert(relayer, balance, validity);
    }

    // Get balances that need refresh (expired or missing)
    pub async fn get_relayers_with_stale_balances(&self, relayers: &[Felt]) -> Vec<Felt> {
        relayers
            .iter()
            .filter(|relayer| self.balances.get_if_not_stale(relayer).is_none())
            .cloned()
            .collect()
    }
}
