use std::collections::HashMap;
use std::sync::Arc;

use paymaster_starknet::math::felt_to_u128;
use paymaster_starknet::{Client, Configuration};
use starknet::core::types::{Felt, FunctionCall};
use starknet::macros::selector;
use tokio::sync::RwLock;

use crate::Error;

#[derive(Clone)]
pub struct DecimalsResolver {
    client: Client,

    cache: Arc<RwLock<HashMap<Felt, i64>>>,
}

impl DecimalsResolver {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            client: Client::new(configuration),

            cache: Arc::default(),
        }
    }

    pub async fn resolve_decimals(&self, token: &Felt) -> Result<i64, Error> {
        if let Some(decimals) = self.resolve_from_cache(token).await {
            return Ok(decimals);
        }

        self.resolve_from_starknet(token).await
    }

    async fn resolve_from_cache(&self, token: &Felt) -> Option<i64> {
        self.cache.read().await.get(token).cloned()
    }

    async fn resolve_from_starknet(&self, token: &Felt) -> Result<i64, Error> {
        let results = self
            .client
            .call(&FunctionCall {
                contract_address: *token,
                entry_point_selector: selector!("decimals"),
                calldata: vec![],
            })
            .await
            .map_err(|_| Error::InvalidDecimals(*token))?;

        let value = results.first().cloned().ok_or(Error::InvalidPrice(*token))?;

        let decimals = felt_to_u128(value).map(|x| x as i64).map_err(|_| Error::InvalidPrice(*token))?;

        self.cache.write().await.insert(*token, decimals);
        Ok(decimals)
    }
}
