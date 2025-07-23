use std::collections::HashSet;
use std::time::{Duration, Instant};

use deadpool_redis::redis::{AsyncCommands, ExistenceCheck, RedisWrite, SetExpiry, SetOptions, ToRedisArgs};
use deadpool_redis::Connection;
use futures::StreamExt;
use starknet::core::types::Felt;

use crate::lock::{Error, RelayerLock};

enum LockKey {
    All,
    Address(Felt),
}

impl ToRedisArgs for LockKey {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        match self {
            Self::All => out.write_arg_fmt("relayer-lock:*"),
            Self::Address(x) => out.write_arg_fmt(format!("relayer-lock:{}", x.to_fixed_hex_string())),
        }
    }
}

struct CacheKey(Felt);

impl ToRedisArgs for CacheKey {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        out.write_arg_fmt(format!("relayer-cache:{}", self.0.to_fixed_hex_string()))
    }
}

pub struct RedisRelayerLock {
    expiry: Instant,

    address: Felt,
    nonce: Option<Felt>,
}

impl From<RelayerLock> for RedisRelayerLock {
    fn from(value: RelayerLock) -> Self {
        Self {
            expiry: value.expiry,

            address: value.address,
            nonce: value.nonce,
        }
    }
}

impl Into<RelayerLock> for RedisRelayerLock {
    fn into(self) -> RelayerLock {
        RelayerLock {
            expiry: self.expiry,
            address: self.address,
            nonce: self.nonce,
        }
    }
}

impl RedisRelayerLock {
    pub async fn list_locked(redis: &mut Connection) -> Result<HashSet<Felt>, Error> {
        let keys: HashSet<String> = redis.scan_match(LockKey::All).await?.collect().await;

        let addresses = keys
            .iter()
            .filter_map(|x: &String| x.strip_prefix("relayer-lock:"))
            .filter_map(|x| Felt::from_hex(x).ok())
            .collect();

        Ok(addresses)
    }

    pub async fn lock(redis: &mut Connection, relayer: Felt) -> Result<Self, Error> {
        Self::lock_with_expiry(redis, relayer, 5).await
    }

    async fn lock_with_expiry(redis: &mut Connection, relayer: Felt, expiry: u64) -> Result<Self, Error> {
        let lock_key = LockKey::Address(relayer);
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(expiry));

        if !redis.set_options(lock_key, Vec::<u8>::new(), options).await? {
            return Err(Error::AlreadyLocked);
        }

        let cache_key = CacheKey(relayer);
        let nonce: Option<Felt> = redis
            .get(cache_key)
            .await
            .map(|x: Vec<u8>| serde_json::from_slice(&x).ok())?
            .unwrap_or(None);

        Ok(Self {
            expiry: Instant::now() + Duration::from_secs(expiry),
            address: relayer,
            nonce,
        })
    }

    // TODO: update redis dep
    #[allow(dependency_on_unit_never_type_fallback)]
    pub async fn unlock(self, redis: &mut Connection) -> Result<(), Error> {
        let cache_key = CacheKey(self.address);
        if let Ok(value) = serde_json::to_vec(&self.nonce) {
            redis.set_ex(cache_key, value, 60).await?;
        }

        let lock_key = LockKey::Address(self.address);
        redis.del(lock_key).await?;

        Ok(())
    }

    // TODO: update redis dep
    #[allow(dependency_on_unit_never_type_fallback)]
    pub async fn unlock_with_expiry(self, redis: &mut Connection, expiry: u64) -> Result<(), Error> {
        let lock_key = LockKey::Address(self.address);
        redis.set_ex(lock_key, Vec::<u8>::new(), expiry).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use deadpool_redis::{Config, Pool, Runtime};
    use paymaster_common::concurrency::ConcurrentExecutor;
    use paymaster_common::task;
    use starknet::core::types::Felt;
    use starknet::macros::felt;
    use testcontainers::core::{IntoContainerPort, WaitFor};
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{ContainerAsync, GenericImage};
    use tokio::time;

    use crate::lock::shared::lock::RedisRelayerLock;

    type RedisContainer = ContainerAsync<GenericImage>;

    async fn redis_container() -> RedisContainer {
        GenericImage::new("redis", "7")
            .with_exposed_port(6379.tcp())
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
    async fn list_lock_when_empty() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        let locks = RedisRelayerLock::list_locked(&mut connection).await.unwrap();
        assert!(locks.is_empty());
    }

    #[tokio::test]
    async fn lock_relayer_works_properly() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        let lock = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
        assert_eq!(lock.address, felt!("0x0"));
        assert_eq!(lock.nonce, None);

        let locks = RedisRelayerLock::list_locked(&mut connection).await.unwrap();
        assert_eq!(locks.len(), 1);
        assert!(locks.contains(&felt!("0x0")))
    }

    #[tokio::test]
    async fn unlock_relayer_works_properly() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        let lock = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
        assert_eq!(lock.address, felt!("0x0"));
        assert_eq!(lock.nonce, None);

        lock.unlock(&mut connection).await.unwrap();

        let locks = RedisRelayerLock::list_locked(&mut connection).await.unwrap();
        assert!(locks.is_empty());
    }

    #[tokio::test]
    async fn double_lock_should_fail() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
        let result = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await;

        assert!(result.is_err())
    }

    #[tokio::test]
    async fn release_lock_should_cache_nonce() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        let mut lock = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
        lock.nonce = Some(felt!("0x42"));

        lock.unlock(&mut connection).await.unwrap();

        let lock = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
        assert_eq!(lock.nonce, Some(felt!("0x42")))
    }

    #[tokio::test]
    async fn lock_with_expiry_works_properly() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut connection = pool.get().await.unwrap();

        let _ = RedisRelayerLock::lock_with_expiry(&mut connection, felt!("0x0"), 1)
            .await
            .unwrap();
        time::sleep(Duration::from_secs(3)).await;

        let _ = RedisRelayerLock::lock(&mut connection, felt!("0x0")).await.unwrap();
    }

    #[tokio::test]
    async fn multiple_concurrent_lock_unlock_works_properly() {
        let container = redis_container().await;
        let pool = redis_pool(&container).await;

        let mut executor = ConcurrentExecutor::new(pool.clone(), 8);
        for lock_id in 0..100 {
            executor.register(task!(|pool| {
                let mut connection = pool.get().await.unwrap();
                let lock_address = Felt::from(lock_id);

                let lock = RedisRelayerLock::lock(&mut connection, lock_address).await.unwrap();

                let locks = RedisRelayerLock::list_locked(&mut connection).await.unwrap();
                assert!(locks.contains(&lock_address));

                lock.unlock(&mut connection).await.unwrap();

                let locks = RedisRelayerLock::list_locked(&mut connection).await.unwrap();
                assert!(!locks.contains(&lock_address));

                lock_id
            }));
        }

        let results = executor.execute().await.unwrap();
        assert_eq!(results.len(), 100);
    }
}
