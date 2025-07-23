use std::collections::HashSet;
use std::fmt::Debug;

use async_trait::async_trait;
use starknet::core::types::Felt;

use crate::lock::{Error, RelayerLock};

#[async_trait]
pub trait MockLockLayer: 'static + Debug + Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    async fn count_enabled_relayers(&self) -> usize {
        unimplemented!()
    }
    async fn set_enabled_relayers(&self, _relayers: &HashSet<Felt>) {
        unimplemented!()
    }
    async fn lock_relayer(&self) -> Result<RelayerLock, Error> {
        unimplemented!()
    }
    async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), Error> {
        unimplemented!()
    }
    async fn release_relayer_delayed(&self, _lock: RelayerLock, _delay: u64) -> Result<(), Error> {
        unimplemented!()
    }
}
