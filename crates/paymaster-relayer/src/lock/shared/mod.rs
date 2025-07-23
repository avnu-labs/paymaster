use std::collections::HashSet;
use std::sync::Arc;

use deadpool_redis::{Config, Connection, Pool, Runtime};
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use tokio::sync::RwLock;

use crate::lock::shared::lock::RedisRelayerLock;
use crate::lock::{Error, RelayerLock};
use crate::rebalancing::RelayerManagerConfiguration;

pub mod lock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisParameters {
    endpoint: String,
}

#[derive(Clone)]
pub struct SharedLockLayer {
    redis: Pool,

    relayers: Arc<RwLock<HashSet<Felt>>>,
}

impl SharedLockLayer {
    pub fn new(configuration: &RelayerManagerConfiguration, params: &RedisParameters) -> Self {
        Self {
            redis: Config::from_url(&params.endpoint)
                .create_pool(Some(Runtime::Tokio1))
                .expect("invalid client"),

            relayers: Arc::new(RwLock::new(configuration.relayers.addresses.iter().cloned().collect())),
        }
    }

    pub async fn count_enabled_relayers(&self) -> usize {
        let enabled_relayers = self.relayers.read().await;
        enabled_relayers.len()
    }

    pub async fn set_enabled_relayers(&self, relayers: &HashSet<Felt>) {
        let mut enabled_relayers = self.relayers.write().await;
        *enabled_relayers = relayers.clone()
    }

    pub async fn lock_relayer(&self) -> Result<RelayerLock, Error> {
        let mut connection = self.get_redis_connection().await?;
        let relayers = self.relayers.read().await;

        let locked_relayers = RedisRelayerLock::list_locked(&mut connection).await?;

        let mut available_relayers: Vec<Felt> = relayers.difference(&locked_relayers).cloned().collect();

        // Shuffle the list to reduce collision when picking relayers concurrently. Also prevent picking
        // always the same relayer
        available_relayers.shuffle(&mut rng());

        for relayer_address in available_relayers {
            match RedisRelayerLock::lock(&mut connection, relayer_address).await {
                Ok(lock) => return Ok(lock.into()),
                Err(_) => continue,
            }
        }

        Err(Error::LockUnavailable)
    }

    pub async fn release_relayer(&self, lock: RelayerLock) -> Result<(), Error> {
        let mut connection = self.get_redis_connection().await?;
        let redis_lock: RedisRelayerLock = lock.into();

        redis_lock.unlock(&mut connection).await
    }

    pub async fn release_relayer_delayed(&self, lock: RelayerLock, delay: u64) -> Result<(), Error> {
        let mut connection = self.get_redis_connection().await?;
        let redis_lock: RedisRelayerLock = lock.into();

        redis_lock.unlock_with_expiry(&mut connection, delay).await
    }
}

impl SharedLockLayer {
    async fn get_redis_connection(&self) -> Result<Connection, Error> {
        let result = self.redis.get().await?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use deadpool_redis::{Config, Pool, Runtime};
    use paymaster_common::concurrency::ConcurrentExecutor;
    use paymaster_common::task;
    use paymaster_starknet::ContractAddress;
    use starknet::core::types::Felt;
    use testcontainers::core::{ContainerPort, WaitFor};
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{ContainerAsync, GenericImage};
    use tokio::sync::{Mutex, RwLock};
    use tokio::time;

    use crate::lock::shared::SharedLockLayer;
    use crate::lock::Duration;

    type RedisContainer = ContainerAsync<GenericImage>;

    async fn redis_container() -> RedisContainer {
        GenericImage::new("redis", "7")
            .with_exposed_port(ContainerPort::Tcp(6379))
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .start()
            .await
            .unwrap()
    }

    async fn redis_pool(container: &RedisContainer) -> Pool {
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let redis_url = format!("redis://127.0.0.1:{}", port);
        let cfg = Config::from_url(redis_url);

        cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
    }

    #[tokio::test]
    async fn multiple_concurrent_lock_unlock_works_properly() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let layer = SharedLockLayer {
            redis: pool,
            relayers: Arc::new(RwLock::new((0..10).map(Felt::from).collect())),
        };

        let mut executor = ConcurrentExecutor::new(layer.clone(), 8);
        for _ in 0..200 {
            executor.register(task!(|layer| {
                let relayer = layer.lock_relayer().await?;
                layer.release_relayer(relayer).await
            }));
        }

        let results = executor.execute().await.unwrap();
        assert!(results.iter().all(|x| x.is_ok()));
    }

    #[tokio::test]
    async fn concurrent_access_is_sound() {
        #[derive(Clone)]
        struct Context {
            active: Arc<Mutex<HashSet<ContractAddress>>>,
            layer: SharedLockLayer,
        }

        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let layer = SharedLockLayer {
            redis: pool,
            relayers: Arc::new(RwLock::new((0..8).map(Felt::from).collect())),
        };

        let ctx = Context {
            active: Arc::new(Mutex::new(HashSet::new())),
            layer,
        };

        let mut executor = ConcurrentExecutor::new(ctx, 8);
        for _ in 0..100 {
            executor.register(task!(|ctx| {
                let Ok(lock) = ctx.layer.lock_relayer().await else { return Ok(()) };

                let mut active = ctx.active.lock().await;
                if active.contains(&lock.address) {
                    return Err(());
                }

                active.insert(lock.address);
                time::sleep(Duration::from_millis(100)).await;

                active.remove(&lock.address);
                let _ = ctx.layer.release_relayer(lock).await;

                Ok(())
            }));
        }

        let results: Result<(), ()> = executor.execute().await.unwrap().into_iter().collect();

        assert!(results.is_ok())
    }
}
