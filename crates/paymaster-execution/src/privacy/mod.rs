use std::time::Duration;

use paymaster_common::cache::ExpirableCache;
use starknet::core::types::{Felt, FunctionCall};
use starknet::macros::selector;
use tracing::warn;

use crate::starknet::Client as Starknet;
use crate::Error;

#[derive(Clone)]
pub struct PrivacyPoolClient {
    starknet: Starknet,
    cache: ExpirableCache<Felt, u128>,
    cache_ttl: Duration,
}

impl PrivacyPoolClient {
    pub fn new(starknet: Starknet, capacity: u64) -> Self {
        Self {
            starknet,
            cache: ExpirableCache::new(capacity),
            cache_ttl: Duration::from_secs(5 * 60),
        }
    }

    pub async fn get_fee_amount(&self, pool_address: Felt) -> Result<u128, Error> {
        if let Some(cached) = self.cache.get_if_not_stale(&pool_address) {
            return Ok(cached);
        }

        let call = FunctionCall {
            contract_address: pool_address,
            entry_point_selector: selector!("get_fee_amount"),
            calldata: vec![],
        };

        match self.starknet.call(&call).await {
            Ok(result) => {
                let felt = result.first().cloned().ok_or_else(|| Error::Execution("get_fee_amount returned empty response".into()))?;
                let fee_amount: u128 =
                    felt.try_into().map_err(|_| Error::Execution("get_fee_amount returned value exceeding u128".into()))?;
                self.cache.insert(pool_address, fee_amount, self.cache_ttl);
                Ok(fee_amount)
            }
            Err(e) => {
                if let Some(cached) = self.cache.get_if_not_expired(&pool_address) {
                    warn!("Failed to fetch get_fee_amount for pool {}, using stale cache: {}", pool_address, e);
                    Ok(cached)
                } else {
                    Err(Error::Execution(format!("Failed to fetch get_fee_amount for pool {}: {}", pool_address, e)))
                }
            }
        }
    }
}
