use std::ops::Deref;
use std::time::Duration;

use moka::sync::Cache;
use paymaster_common::cache::ExpirableCache;
use paymaster_common::concurrency::SyncValue;
use paymaster_starknet::transaction::PaymasterVersion;
use paymaster_starknet::{BlockGasPrice, Configuration, ContractAddress};
use starknet::core::types::Felt;
use tracing::warn;

use crate::execution::ValidationGasOverhead;
use crate::Error;

/// Starknet client with convenience methods used when executing paymaster transaction. This
/// can be shared between threads safely, taking advantages of the internal caching to reduce
/// the number of external calls made.
#[derive(Clone)]
pub struct Client {
    inner: paymaster_starknet::Client,

    // Cache block price for 10 seconds
    cache_block_price: SyncValue<BlockGasPrice>,

    // Cache median tip for 10 seconds
    cache_median_tip: SyncValue<u64>,

    // Cache account version for 5 minutes
    cache_account_version: ExpirableCache<Felt, PaymasterVersion>,

    // Cache class version
    cache_class_version: Cache<Felt, PaymasterVersion>,

    // Cache account overhead
    cache_overhead: Cache<Felt, ValidationGasOverhead>,
}

impl Deref for Client {
    type Target = paymaster_starknet::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Client {
    /// Creates a new client given a [`configuration`]
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            inner: paymaster_starknet::Client::new(configuration),

            cache_block_price: SyncValue::new(Duration::from_secs(10)),
            cache_median_tip: SyncValue::new(Duration::from_secs(10)),
            cache_account_version: ExpirableCache::new(1024),
            cache_class_version: Cache::new(128),
            cache_overhead: Cache::new(1024),
        }
    }

    /// Resolve the paymaster version associated to the [`user`] account. This function relies on a
    /// cache whose entries expires every 5 minutes so subsequent calls for the same user are resolved
    /// without any external calls.
    ///
    /// Detection strategy:
    /// 1. Try SRC-5 `supports_interface` for SNIP-9 V1/V2 interface IDs
    /// 2. If SRC-5 finds nothing (returns None), fall back to ABI inspection of the account's class
    /// 3. If SRC-5 call itself errors, use cached value or fall back to ABI inspection
    pub async fn resolve_paymaster_version_from_account(&self, user: ContractAddress) -> Result<PaymasterVersion, Error> {
        if let Some(value) = self.cache_account_version.get_if_not_stale(&user) {
            return Ok(value);
        }

        let version = match PaymasterVersion::fetch_supported_version(self, user).await {
            Ok(supported_version) => match supported_version.maximum_version() {
                Some(v) => v,
                None => {
                    // SRC-5 didn't find SNIP-9 interfaces, fall back to ABI inspection
                    warn!("SRC-5 detection found no SNIP-9 support for {}, trying ABI fallback", user.to_fixed_hex_string());
                    self.resolve_version_from_account_class(user).await?
                },
            },
            Err(e) => {
                if let Some(v) = self.cache_account_version.get_if_not_expired(&user) {
                    return Ok(v);
                }
                warn!("SRC-5 detection failed for {}: {}, trying ABI fallback", user.to_fixed_hex_string(), e);
                self.resolve_version_from_account_class(user).await?
            },
        };

        self.cache_account_version.insert(user, version, Duration::from_secs(5 * 60));
        Ok(version)
    }

    /// Fall back to ABI-based version detection by fetching the account's class hash and inspecting its ABI
    async fn resolve_version_from_account_class(&self, user: ContractAddress) -> Result<PaymasterVersion, Error> {
        let class_hash = self.inner.fetch_class_hash_at(user).await?;
        self.resolve_paymaster_version_from_class(class_hash).await
    }

    /// Resolve the paymaster version associated to the [`class_hash`]. This function relies on a
    /// cache so subsequent calls for the same class_hash are resolved without any external calls.
    pub async fn resolve_paymaster_version_from_class(&self, class_hash: Felt) -> Result<PaymasterVersion, Error> {
        if let Some(value) = self.cache_class_version.get(&class_hash) {
            return Ok(value);
        }

        let class = self.inner.fetch_class(class_hash).await?;
        let version = PaymasterVersion::from_class(&class)?;

        self.cache_class_version.insert(class_hash, version);

        Ok(version)
    }

    /// Resolve the gas overhead associated to the [`user`] account. This function relies on a cache so subsequent
    /// call for the same user are resolved without any external calls
    pub async fn resolve_gas_overhead(&self, user: Felt) -> Result<ValidationGasOverhead, Error> {
        if let Some(value) = self.cache_overhead.get(&user) {
            return Ok(value);
        }

        let overhead = ValidationGasOverhead::fetch(self, user).await?;
        self.cache_overhead.insert(user, overhead);

        Ok(overhead)
    }

    /// Fetch the current block gas price. This function relies on a cache that expires every 10s so
    /// during that time frame calling it won't induce external calls
    pub async fn fetch_block_gas_price(&self) -> Result<BlockGasPrice, Error> {
        let client = self.inner.clone();
        let price = self
            .cache_block_price
            .read_or_refresh(|| Box::pin(async move { client.fetch_block_gas_price().await }))
            .await?;

        Ok(price)
    }

    /// Fetch the median tip of the latest block. This function relies on a cache that expires every 10s so
    /// during that time frame calling it won't induce external calls
    pub async fn fetch_median_tip(&self) -> Result<u64, Error> {
        let client = self.inner.clone();
        let tip = self
            .cache_median_tip
            .read_or_refresh(|| Box::pin(async move { client.fetch_block_median_tip().await }))
            .await?;

        Ok(tip)
    }
}
