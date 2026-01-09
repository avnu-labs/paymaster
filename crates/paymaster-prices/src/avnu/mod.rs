use std::time::Duration;

use paymaster_common::cache::ExpirableCache;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::normalize_felt;
use paymaster_starknet::Configuration as StarknetConfiguration;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HTTPClient, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with::serde_as;
use starknet::core::types::Felt;

use crate::decimals::DecimalsResolver;
use crate::{Error, PriceClient, PriceOracleConfiguration, TokenPrice};

pub const DEFAULT_AVNU_PRICE_SEPOLIA_ENDPOINT: &str = "https://sepolia.api.avnu.fi";
pub const DEFAULT_AVNU_PRICE_MAINNET_ENDPOINT: &str = "https://starknet.api.avnu.fi";

#[serde_as]
#[derive(Deserialize, Clone, Copy, Debug)]
struct Price {
    #[serde(rename = "usdPrice")]
    pub price_in_usd: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AVNUPriceClientConfiguration {
    pub endpoint: String,
    pub api_key: String,
    pub starknet: StarknetConfiguration,
}

impl From<AVNUPriceClientConfiguration> for PriceOracleConfiguration {
    fn from(value: AVNUPriceClientConfiguration) -> Self {
        Self::AVNU(value)
    }
}

#[derive(Clone)]
pub struct AVNUPriceOracle {
    endpoint: String,
    client: HTTPClient,
    cache: ExpirableCache<Felt, Price>,

    resolver: DecimalsResolver,
}

impl From<AVNUPriceOracle> for PriceClient {
    fn from(value: AVNUPriceOracle) -> Self {
        Self::AVNU(value)
    }
}

impl AVNUPriceOracle {
    pub fn new(configuration: &AVNUPriceClientConfiguration) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&configuration.api_key).expect("invalid api key"));

        Self {
            endpoint: configuration.endpoint.clone(),
            client: HTTPClient::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(3))
                .build()
                .expect("invalid client"),

            resolver: DecimalsResolver::new(&configuration.starknet),
            cache: ExpirableCache::new(128),
        }
    }

    pub async fn fetch_token(&self, address: &Felt) -> Result<TokenPrice, Error> {
        let strk_price = self.fetch_token_by_address(&Token::STRK_ADDRESS).await?;
        if !strk_price.price_in_usd.is_normal() {
            return Err(Error::InvalidPrice(*address));
        }

        let token_price = self.fetch_token_by_address(address).await?;
        let decimals = self.resolver.resolve_decimals(address).await?;

        Ok(TokenPrice {
            address: *address,
            decimals,
            price_in_strk: normalize_felt(token_price.price_in_usd / strk_price.price_in_usd, 18),
        })
    }

    async fn fetch_token_by_address(&self, address: &Felt) -> Result<Price, Error> {
        if let Some(price) = self.fetch_token_from_cache(address) {
            return Ok(price);
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

        let price = serde_json::from_str::<Vec<Price>>(&text)
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
    use paymaster_starknet::{ChainID, DEFAULT_SEPOLIA_RPC_ENDPOINT};
    use starknet::core::types::Felt;

    use crate::avnu::{AVNUPriceClientConfiguration, AVNUPriceOracle};

    #[ignore] // Require API key
    #[tokio::test]
    async fn should_return_tokens() {
        // Given
        let oracle = AVNUPriceOracle::new(&AVNUPriceClientConfiguration {
            endpoint: DEFAULT_SEPOLIA_RPC_ENDPOINT.to_string(),
            api_key: String::from("dummy-key"),
            starknet: paymaster_starknet::Configuration {
                endpoint: DEFAULT_SEPOLIA_RPC_ENDPOINT.to_string(),
                chain_id: ChainID::Sepolia,
                timeout: 10,
                fallbacks: vec![],
            },
        });

        // When
        let result = oracle.fetch_token(&Token::ETH_ADDRESS).await.unwrap();

        // Then
        assert_eq!(result.address, Token::ETH_ADDRESS);
        assert!(result.price_in_strk > Felt::ZERO);
    }
}
