use std::collections::HashSet;
use std::time::{Duration, Instant};

use deadpool_redis::redis::RedisError;
use deadpool_redis::PoolError;
use paymaster_common::{measure_duration, metric};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::types::Felt;
use thiserror::Error;

use crate::lock::seggregated::SeggregatedLockLayer;
use crate::lock::shared::{RedisParameters, SharedLockLayer};
use crate::rebalancing::RelayerManagerConfiguration;

#[cfg(feature = "testing")]
pub mod mock;

pub mod seggregated;
pub mod shared;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Redis(#[from] RedisError),

    #[error(transparent)]
    Connection(#[from] PoolError),

    #[error("already locked")]
    AlreadyLocked,

    #[error("lock is unavailable")]
    LockUnavailable,
}

#[derive(Debug, Clone, Copy)]
pub struct RelayerLock {
    expiry: Instant,

    pub address: Felt,
    pub nonce: Option<Felt>,
}

impl RelayerLock {
    pub fn new(address: Felt, nonce: Option<Felt>, validity: Duration) -> Self {
        Self {
            expiry: Instant::now() + validity,
            address,
            nonce,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expiry
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum LockLayerConfiguration {
    #[cfg(feature = "testing")]
    #[serde(skip)]
    Mock {
        #[serde_as(as = "serde_with::DurationSeconds")]
        retry_timeout: Duration,
        lock_layer: std::sync::Arc<dyn mock::MockLockLayer>,
    },

    Seggregated {
        #[serde_as(as = "serde_with::DurationSeconds")]
        retry_timeout: Duration,
    },
    Shared {
        #[serde_as(as = "serde_with::DurationSeconds")]
        retry_timeout: Duration,
        redis: RedisParameters,
    },
}

#[cfg(feature = "testing")]
impl LockLayerConfiguration {
    pub fn mock<T: mock::MockLockLayer>() -> Self {
        Self::mock_with_timeout::<T>(Duration::from_secs(5))
    }

    pub fn mock_with_timeout<T: mock::MockLockLayer>(retry_timeout: Duration) -> Self {
        Self::Mock {
            retry_timeout,
            lock_layer: std::sync::Arc::new(T::new()),
        }
    }
}

impl LockLayerConfiguration {
    pub fn retry_timeout(&self) -> Duration {
        match self {
            #[cfg(feature = "testing")]
            Self::Mock { retry_timeout, .. } => *retry_timeout,
            Self::Seggregated { retry_timeout } => *retry_timeout,
            Self::Shared { retry_timeout, .. } => *retry_timeout,
        }
    }
}

#[derive(Clone)]
pub enum LockLayer {
    #[cfg(feature = "testing")]
    Mock(std::sync::Arc<dyn mock::MockLockLayer>),

    Seggregated(SeggregatedLockLayer),
    Shared(SharedLockLayer),
}

impl LockLayer {
    pub fn new(configuration: &RelayerManagerConfiguration) -> Self {
        match &configuration.relayers.lock {
            LockLayerConfiguration::Shared { redis, .. } => LockLayer::Shared(SharedLockLayer::new(configuration, &redis)),
            LockLayerConfiguration::Seggregated { .. } => LockLayer::Seggregated(SeggregatedLockLayer::new(configuration)),

            #[cfg(feature = "testing")]
            LockLayerConfiguration::Mock { lock_layer, .. } => LockLayer::Mock(lock_layer.clone()),
        }
    }

    #[cfg(feature = "testing")]
    pub fn mock<I: mock::MockLockLayer>() -> Self {
        Self::Mock(std::sync::Arc::new(I::new()))
    }

    pub async fn count_enabled_relayers(&self) -> usize {
        match self {
            #[cfg(feature = "testing")]
            Self::Mock(x) => x.count_enabled_relayers().await,
            Self::Shared(x) => x.count_enabled_relayers().await,
            Self::Seggregated(x) => x.count_enabled_relayers().await,
        }
    }

    pub async fn set_enabled_relayers(&self, relayers: &HashSet<Felt>) {
        match self {
            #[cfg(feature = "testing")]
            Self::Mock(x) => x.set_enabled_relayers(relayers).await,
            Self::Shared(x) => x.set_enabled_relayers(relayers).await,
            Self::Seggregated(x) => x.set_enabled_relayers(relayers).await,
        }
    }

    pub async fn lock_relayer(&self) -> Result<RelayerLock, Error> {
        let (result, duration) = measure_duration!(match self {
            #[cfg(feature = "testing")]
            Self::Mock(x) => x.lock_relayer().await,
            Self::Shared(x) => x.lock_relayer().await,
            Self::Seggregated(x) => x.lock_relayer().await,
        });

        metric!(counter[relayer_request_duration_milliseconds] = 1, method = "lock_relayer");
        metric!(histogram[relayer_request_duration_milliseconds] = duration.as_millis(), method = "lock_relayer");
        metric!(on error result => counter [ relayer_request_error ] = 1, method = "lock_relayer");

        result
    }

    pub async fn release_relayer(&self, lock: RelayerLock) -> Result<(), Error> {
        let (result, duration) = measure_duration!(match self {
            #[cfg(feature = "testing")]
            Self::Mock(x) => x.release_relayer(lock).await,
            Self::Shared(x) => x.release_relayer(lock).await,
            Self::Seggregated(x) => x.release_relayer(lock).await,
        });

        metric!(counter[relayer_request_duration_milliseconds] = 1, method = "release_relayer");
        metric!(histogram[relayer_request_duration_milliseconds] = duration.as_millis(), method = "release_relayer");
        metric!(on error result => counter [ relayer_request_error ] = 1, method = "release_relayer");

        result
    }

    pub async fn release_relayer_delayed(&self, lock: RelayerLock, delay: u64) -> Result<(), Error> {
        let (result, duration) = measure_duration!(match self {
            #[cfg(feature = "testing")]
            Self::Mock(x) => x.release_relayer_delayed(lock, delay).await,
            Self::Shared(x) => x.release_relayer_delayed(lock, delay).await,
            Self::Seggregated(x) => x.release_relayer_delayed(lock, delay).await,
        });

        metric!(counter[relayer_request_duration_milliseconds] = 1, method = "release_relayer_delayed");
        metric!(
            histogram[relayer_request_duration_milliseconds] = duration.as_millis(),
            method = "release_relayer_delayed"
        );
        metric!(on error result => counter [ relayer_request_error ] = 1, method = "release_relayer_delayed");

        result
    }
}
