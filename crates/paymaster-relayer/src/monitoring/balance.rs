use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use num_traits::ToPrimitive;
use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::service::{Error, Service};
use paymaster_common::{metric, service_check, task};
use paymaster_starknet::constants::Token;
use starknet::core::types::Felt;
use tokio::time;

use crate::Context;

pub struct RelayerBalanceMonitoring {
    context: Context,
    relayers: HashSet<Felt>,
}

#[async_trait]
impl Service for RelayerBalanceMonitoring {
    type Context = Context;

    const NAME: &'static str = "RelayerBalance";

    async fn new(context: Context) -> Self {
        Self {
            relayers: context.configuration.relayers.addresses.iter().cloned().collect(),
            context,
        }
    }

    async fn run(mut self) -> Result<(), Error> {
        let min_balance = self.context.configuration.relayers.min_relayer_balance;

        let mut ticker = time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            let relayer_balances = service_check!(self.fetch_relayer_balances(self.relayers.clone()).await => continue);

            // Update balance cache with fetched balances
            for (relayer, balance) in &relayer_balances {
                self.context.relayers.set_relayer_balance(*relayer, *balance).await;
            }

            let mut enabled_relayers = self.relayers.clone();
            for (relayer, balance) in relayer_balances {
                if balance <= min_balance {
                    enabled_relayers.remove(&relayer);
                }
            }

            self.context.relayers_locks.set_enabled_relayers(&enabled_relayers).await
        }
    }
}

impl RelayerBalanceMonitoring {
    #[rustfmt::skip]
    async fn fetch_relayer_balances(&self, relayers: HashSet<Felt>) -> Result<HashMap<Felt, Felt>, Error> {
        let mut executor = ConcurrentExecutor::new(self.context.clone(), 8);

        for relayer in relayers {
            executor.register(task!(|ctx| {
                ctx.starknet.fetch_balance(Token::strk(ctx.starknet.chain_id()).address, relayer).await.map(|x| (relayer, x))
            }));
        }

        let results = executor
            .execute()
            .await
            .map_err(Error::from)?;

        let mut balances = HashMap::new();
        for result in results {
            let (relayer, balance) = service_check!(result => continue);
            let balance_in_strk = balance.to_biguint().to_f64().unwrap_or_default();
            let balance_in_strk_normalized = balance_in_strk / 1e18;

            metric!(gauge [ relayer_balance_in_strk ] = balance_in_strk_normalized, relayer = relayer.to_fixed_hex_string());
            balances.insert(relayer, balance);
        }

        Ok(balances)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, LinkedList};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use paymaster_starknet::constants::{Endpoint, Token};
    use paymaster_starknet::{ChainID, Configuration as StarknetConfiguration, StarknetAccountConfiguration};
    use starknet::core::types::Felt;
    use starknet::macros::felt;
    use tokio::sync::RwLock;

    use crate::lock::mock::MockLockLayer;
    use crate::lock::LockLayerConfiguration;
    use crate::rebalancing::{OptionalRebalancingConfiguration, RelayerManagerConfiguration};
    use crate::RelayersConfiguration;

    #[derive(Default, Debug)]
    pub struct Lock(Arc<RwLock<LinkedList<Felt>>>);

    #[async_trait]
    impl MockLockLayer for Lock {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self(Arc::new(RwLock::new(LinkedList::new())))
        }

        async fn count_enabled_relayers(&self) -> usize {
            self.0.read().await.len()
        }

        async fn set_enabled_relayers(&self, relayers: &HashSet<Felt>) {
            self.0.write().await.extend(relayers)
        }
    }

    fn configuration() -> RelayerManagerConfiguration {
        RelayerManagerConfiguration {
            starknet: StarknetConfiguration {
                chain_id: ChainID::Sepolia,
                endpoint: Endpoint::default_rpc_url(&ChainID::Sepolia).to_string(),
                fallbacks: vec![],
                timeout: 10,
            },
            supported_tokens: HashSet::from([Token::usdc(&ChainID::Sepolia).address]),
            gas_tank: StarknetAccountConfiguration {
                address: felt!("0x0"),
                private_key: felt!("0x0"),
            },
            relayers: RelayersConfiguration {
                min_relayer_balance: Felt::ZERO,
                private_key: felt!("0x0"),
                addresses: vec![felt!("0x0")],
                lock: LockLayerConfiguration::mock_with_timeout::<Lock>(Duration::from_secs(5)),
                rebalancing: OptionalRebalancingConfiguration::initialize(None),
            },
        }
    }
}
