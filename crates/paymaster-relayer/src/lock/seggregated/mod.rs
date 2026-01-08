use std::collections::{HashMap, HashSet};
use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, Instant};

use paymaster_starknet::ContractAddress;
use rand::prelude::IndexedRandom;
use rand::rng;
use starknet::core::types::Felt;
use tokio::sync::Mutex;

use crate::lock::{Error, RelayerLock};
use crate::RelayerManagerConfiguration;

#[derive(Clone, Copy)]
struct SeggregatedRelayerLock {
    address: ContractAddress,
    nonce: Option<Felt>,
    enabled: bool,
    cooldown: Instant,
}

impl SeggregatedRelayerLock {
    pub fn new(address: ContractAddress) -> Self {
        Self {
            address,
            nonce: None,
            enabled: true,
            cooldown: Instant::now(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.enabled && self.cooldown <= Instant::now()
    }
}

impl From<SeggregatedRelayerLock> for RelayerLock {
    fn from(value: SeggregatedRelayerLock) -> Self {
        Self::new(value.address, value.nonce, Duration::from_secs(60))
    }
}

#[derive(Clone)]
pub struct SeggregatedLockLayer {
    relayer_by_address: Arc<HashMap<ContractAddress, usize>>,
    relayers: Arc<Mutex<Vec<SeggregatedRelayerLock>>>,
}

impl SeggregatedLockLayer {
    pub fn new(configuration: &RelayerManagerConfiguration) -> Self {
        let mut relayers = vec![];
        for address in &configuration.relayers.addresses {
            relayers.push(SeggregatedRelayerLock::new(*address))
        }

        Self {
            relayer_by_address: relayers
                .iter()
                .enumerate()
                .map(|(i, l)| (l.address, i))
                .collect::<HashMap<ContractAddress, usize>>()
                .into(),

            relayers: Arc::new(Mutex::new(relayers)),
        }
    }

    pub async fn count_enabled_relayers(&self) -> usize {
        let enabled_relayers = self.relayers.lock().await;
        enabled_relayers.iter().filter(|x| x.enabled).count()
    }

    pub async fn set_enabled_relayers(&self, relayers: &HashSet<Felt>) {
        let mut enabled_relayers = self.relayers.lock().await;
        enabled_relayers
            .iter_mut()
            .for_each(|x| x.enabled = relayers.contains(&x.address))
    }

    pub async fn lock_relayer(&self) -> Result<RelayerLock, Error> {
        let mut relayers = self.relayers.lock().await;

        let available_relayers: Vec<usize> = relayers
            .iter()
            .enumerate()
            .filter(|(_, x)| x.is_available())
            .map(|(i, _)| i)
            .collect();

        let lock_index = available_relayers.choose(&mut rng()).cloned().ok_or(Error::LockUnavailable)?;

        relayers[lock_index].cooldown = Instant::now().add(Duration::from_secs(5));
        Ok(relayers[lock_index].into())
    }

    pub async fn release_relayer(&self, lock: RelayerLock) -> Result<(), Error> {
        let lock_index = self.relayer_by_address.get(&lock.address).ok_or(Error::LockUnavailable)?;

        let mut relayers = self.relayers.lock().await;
        relayers[*lock_index].cooldown = Instant::now();
        relayers[*lock_index].nonce = lock.nonce;

        Ok(())
    }

    pub async fn release_relayer_delayed(&self, lock: RelayerLock, delay: u64) -> Result<(), Error> {
        let lock_index = self.relayer_by_address.get(&lock.address).ok_or(Error::LockUnavailable)?;

        let mut relayers = self.relayers.lock().await;
        relayers[*lock_index].cooldown = Instant::now().add(Duration::from_secs(delay));
        relayers[*lock_index].nonce = lock.nonce;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    use paymaster_common::concurrency::ConcurrentExecutor;
    use paymaster_common::task;
    use paymaster_starknet::constants::Token;
    use paymaster_starknet::{ChainID, Configuration as StarknetConfiguration, ContractAddress, StarknetAccountConfiguration};
    use starknet::core::types::Felt;
    use starknet::macros::felt;
    use tokio::sync::Mutex;
    use tokio::time;

    use crate::lock::seggregated::SeggregatedLockLayer;
    use crate::lock::LockLayerConfiguration;
    use crate::rebalancing::OptionalRebalancingConfiguration;
    use crate::{RelayerManagerConfiguration, RelayersConfiguration};
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::Configuration as PriceConfiguration;

    #[derive(Debug)]
    pub struct MockPrice;

    impl MockPriceOracle for MockPrice {
        fn new() -> Self {
            Self
        }
    }

    fn locking_layer(relayers: Vec<Felt>) -> SeggregatedLockLayer {
        SeggregatedLockLayer::new(&RelayerManagerConfiguration {
            starknet: StarknetConfiguration {
                endpoint: "dummy".to_string(),
                chain_id: ChainID::Sepolia,
                fallbacks: vec![],
                timeout: 10,
            },
            supported_tokens: HashSet::from([Token::usdc(&ChainID::Sepolia).address]),
            gas_tank: StarknetAccountConfiguration {
                address: felt!("0x0"),
                private_key: felt!("0x0"),
            },
            relayers: RelayersConfiguration {
                min_relayer_balance: felt!("0x0"),
                private_key: Felt::ZERO,
                addresses: relayers,
                lock: LockLayerConfiguration::Seggregated {
                    retry_timeout: Duration::from_secs(5),
                },
                rebalancing: OptionalRebalancingConfiguration::initialize(None),
            },
            price: PriceConfiguration::mock::<MockPrice>(),
        })
    }

    #[tokio::test]
    async fn enable_relayers_works_properly() {
        let layer = locking_layer(vec![felt!("0x0"), felt!("0x1")]);

        assert_eq!(layer.count_enabled_relayers().await, 2);

        layer.set_enabled_relayers(&HashSet::new()).await;
        assert_eq!(layer.count_enabled_relayers().await, 0);

        layer.set_enabled_relayers(&HashSet::from([felt!("0x0")])).await;
        assert_eq!(layer.count_enabled_relayers().await, 1)
    }

    #[tokio::test]
    async fn lock_unlock_relayers_works_properly() {
        let layer = locking_layer(vec![felt!("0x0"), felt!("0x1")]);

        let lock_1 = layer.lock_relayer().await.unwrap();
        let _ = layer.lock_relayer().await.unwrap();

        let fail_lock_3 = layer.lock_relayer().await;
        assert!(fail_lock_3.is_err());

        layer.release_relayer(lock_1).await.unwrap();

        let _ = layer.lock_relayer().await.unwrap();
    }

    #[tokio::test]
    async fn lock_unlock_delayed_relayers_works_properly() {
        let layer = locking_layer(vec![felt!("0x0")]);

        let lock_1 = layer.lock_relayer().await.unwrap();
        layer.release_relayer_delayed(lock_1, 3).await.unwrap();

        let failed_lock = layer.lock_relayer().await;
        assert!(failed_lock.is_err());

        time::sleep(Duration::from_secs(3)).await;

        let _ = layer.lock_relayer().await.unwrap();
    }

    #[tokio::test]
    async fn multiple_concurrent_lock_unlock_works_properly() {
        let layer = locking_layer((0..8).map(Felt::from).collect());

        let mut executor = ConcurrentExecutor::new(layer.clone(), 8);
        for _ in 0..200 {
            executor.register(task!(|lock_layer| {
                let relayer = lock_layer.lock_relayer().await?;
                lock_layer.release_relayer(relayer).await
            }));
        }

        let results = executor.execute().await.unwrap();
        assert!(results.iter().all(|x| x.is_ok()));
    }

    #[tokio::test]
    async fn concurrent_access_is_sound() {
        #[derive(Clone)]
        struct Context {
            enabled: Arc<Mutex<HashSet<ContractAddress>>>,
            layer: SeggregatedLockLayer,
        }

        let layer = locking_layer((0..8).map(Felt::from).collect());

        let ctx = Context {
            enabled: Arc::new(Mutex::new(HashSet::new())),
            layer,
        };

        let mut executor = ConcurrentExecutor::new(ctx, 8);
        for _ in 0..100 {
            executor.register(task!(|ctx| {
                let Ok(lock) = ctx.layer.lock_relayer().await else { return Ok(()) };

                let mut enabled = ctx.enabled.lock().await;
                if enabled.contains(&lock.address) {
                    return Err(());
                }

                enabled.insert(lock.address);
                time::sleep(Duration::from_millis(100)).await;

                enabled.remove(&lock.address);
                let _ = ctx.layer.release_relayer(lock).await;

                Ok(())
            }));
        }

        let results: Result<(), ()> = executor.execute().await.unwrap().into_iter().collect();

        assert!(results.is_ok())
    }
}
