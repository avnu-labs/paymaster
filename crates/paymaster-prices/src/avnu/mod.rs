use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_common::concurrency::SyncValue;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HTTPClient, Url};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::Felt;
use tokio::sync::RwLock;

use crate::{Client, Error, PriceOracle, TokenPrice};

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ImpulseTokenPrice {
    #[serde_as(as = "UfeHex")]
    pub address: Felt,

    pub decimals: i64,

    #[serde_as(as = "UfeHex")]
    #[serde(rename = "priceInSTRK")]
    pub price_in_strk: Felt,
}

impl From<ImpulseTokenPrice> for TokenPrice {
    fn from(value: ImpulseTokenPrice) -> Self {
        Self {
            address: value.address,
            decimals: value.decimals,
            price_in_strk: value.price_in_strk,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AVNUPriceClientConfiguration {
    pub endpoint: String,
    pub api_key: Option<String>,
}

#[derive(Clone)]
pub struct AVNUPriceOracle {
    endpoint: String,
    client: HTTPClient,
    cache: Arc<RwLock<HashMap<Felt, SyncValue<TokenPrice>>>>,
}

impl From<AVNUPriceOracle> for Client {
    fn from(value: AVNUPriceOracle) -> Self {
        Self::AVNU(value)
    }
}

impl AVNUPriceOracle {
    pub fn new(configuration: &AVNUPriceClientConfiguration) -> Self {
        let mut headers = HeaderMap::new();
        if let Some(ref api_key) = configuration.api_key {
            headers.insert("x-api-key", HeaderValue::from_str(api_key).expect("invalid api key"));
        }

        Self {
            endpoint: configuration.endpoint.clone(),

            client: HTTPClient::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(3))
                .build()
                .expect("invalid client"),

            cache: Arc::default(),
        }
    }
}

#[async_trait]
impl PriceOracle for AVNUPriceOracle {
    async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, Error> {
        let cached_token = self.fetch_token_from_cache(address).await;

        cached_token
            .read_or_refresh({
                let this = self.clone();
                move || Box::pin(async move { this.fetch_token_from_impulse(address).await })
            })
            .await
    }
}

impl AVNUPriceOracle {
    async fn fetch_token_from_cache(&self, address: Felt) -> SyncValue<TokenPrice> {
        if let Some(value) = self.cache.read().await.get(&address) {
            return value.clone();
        }

        let mut write_lock = self.cache.write().await;
        write_lock
            .entry(address)
            .or_insert(SyncValue::new(Duration::from_secs(60)))
            .clone()
    }

    async fn fetch_token_from_impulse(&self, address: Felt) -> Result<TokenPrice, Error> {
        let url = Url::parse(&self.endpoint)
            .map_err(|e| Error::URL(e.to_string()))?
            .query_pairs_mut()
            .append_pair("token", &format!("0x{:x}", address))
            .finish()
            .clone();

        // Fetch
        let response = self.client.get(url.clone()).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(Error::Internal(format!("Impulse request error url={} status={}, body={}", url, status, text)));
        }

        let tokens: Vec<ImpulseTokenPrice> = serde_json::from_str(&text).map_err(|e| Error::Format(e.to_string()))?;

        tokens
            .first()
            .cloned()
            .map(Into::<TokenPrice>::into)
            .ok_or(Error::Internal("Token not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use paymaster_starknet::constants::{Endpoint, Token};
    use paymaster_starknet::ChainID;

    use super::*;

    fn client() -> Client {
        AVNUPriceOracle::new(&AVNUPriceClientConfiguration {
            endpoint: Endpoint::default_price_url(&ChainID::Sepolia).to_string(),
            api_key: None,
        })
        .into()
    }

    #[tokio::test]
    async fn should_return_tokens() {
        // Given
        let oracle = client();
        let tokens = HashSet::from([Token::eth(&ChainID::Sepolia).address, Token::usdc(&ChainID::Sepolia).address]);

        // When
        let result = oracle.fetch_tokens(&tokens).await.unwrap();

        // Then
        assert_eq!(2, result.len());
    }
}
