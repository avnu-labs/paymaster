use std::time::Duration;

use crate::{Error, PriceClient, TokenPrice};
use paymaster_common::cache::ExpirableCache;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::normalize_felt;
use paymaster_starknet::ChainID;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HTTPClient, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::Felt;

#[serde_as]
#[derive(Deserialize, Clone, Copy, Debug)]
struct Price {
    #[serde_as(as = "UfeHex")]
    #[serde(rename = "tokenAddress")]
    pub address: Felt,

    #[serde(rename = "usdPrice")]
    pub price_in_usd: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AVNUPriceClientConfiguration {
    endpoint: String,
    api_key: Option<String>,
}

impl AVNUPriceClientConfiguration {
    pub fn new(chain_id: ChainID, api_key: Option<String>) -> Self {
        match chain_id {
            ChainID::Sepolia => Self::sepolia(api_key),
            ChainID::Mainnet => Self::mainnet(api_key)
        }
    }

    pub fn sepolia(api_key: Option<String>) -> Self {
        Self {
            endpoint: String::from("https://sepolia.api.avnu.fi"),
            api_key
        }
    }

    pub fn mainnet(api_key: Option<String>) -> Self {
        Self {
            endpoint: String::from("https://starknet.api.avnu.fi"),
            api_key
        }
    }
}

#[derive(Clone)]
pub struct AVNUPriceOracle {
    endpoint: String,
    client: HTTPClient,
    cache: ExpirableCache<Felt, Price>
}

impl From<AVNUPriceOracle> for PriceClient {
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

            cache: ExpirableCache::new(128),
        }
    }

    pub async fn fetch_token(&self, address: &Felt) -> Result<TokenPrice, Error> {
        let strk_price = self.fetch_token_by_address(&Token::STRK).await?;
        if !strk_price.price_in_usd.is_normal() {
            return Err(Error::InvalidPrice(*address))
        }

        let token_price = self.fetch_token_by_address(address).await?;

        Ok(TokenPrice {
            address: *address,
            price_in_strk: normalize_felt(token_price.price_in_usd / strk_price.price_in_usd, 18)
        })
    }

    async fn fetch_token_by_address(&self, address: &Felt) -> Result<Price, Error> {
        if let Some(price) = self.fetch_token_from_cache(address) {
            return Ok(price)
        }

        self.fetch_token_from_avnu(address).await
    }

    fn fetch_token_from_cache(&self, address: &Felt) -> Option<Price> {
        self.cache.get_if_not_expired(address)
    }

    async fn fetch_token_from_avnu(&self, address: &Felt) -> Result<Price, Error> {
        let url = Url::parse(&self.endpoint)
            .and_then(|x| x.join("/v1/tokens/prices"))
            .map_err(|e| Error::URL(e.to_string()))?;

        // Fetch
        let response = self
            .client
            .post(url.clone())
            .json(&json!({ "tokens": [address.to_hex_string()] }))
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(Error::Internal(format!("request error url={} status={}, body={}", url, status, text)));
        }

        let price= serde_json::from_str::<Vec<Price>>(&text)
            .map_err(|e| Error::Format(e.to_string()))?
            .first()
            .cloned()
            .ok_or(Error::InvalidPrice(*address))?;

        self.cache.insert(*address, price, Duration::from_secs(3));
        Ok(price)
    }
}

#[cfg(test)]
mod tests {
    use paymaster_starknet::constants::Token;

    use super::*;

    fn client() -> AVNUPriceOracle {
        AVNUPriceOracle::new(&AVNUPriceClientConfiguration::mainnet(None))
    }

    #[tokio::test]
    async fn should_return_tokens() {
        // Given
        let oracle = client();

        // When
        let result = oracle.fetch_token(&Token::ETH).await.unwrap();

        // Then
        assert_eq!(result.address, Token::ETH);
        assert!(result.price_in_strk > Felt::ZERO);
    }
}
