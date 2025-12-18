use crate::{Error, PriceClient, TokenPrice};
use paymaster_common::cache::ExpirableCache;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::normalize_felt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{ClientBuilder, Url};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoingeckoPriceClientConfiguration {
    pub endpoint: String,
    pub api_key: Option<String>,
}

#[derive(Clone)]
pub struct CoingeckoPriceClient {
    endpoint: String,
    client: reqwest::Client,

    cache: ExpirableCache<Felt, Price>
}

impl From<CoingeckoPriceClient> for PriceClient {
    fn from(value: CoingeckoPriceClient) -> Self {
        Self::Coingecko(value)
    }
}

#[derive(Deserialize)]
struct PriceResponse(HashMap<Felt, Price>);

#[derive(Deserialize, Debug, Clone, Copy)]
struct Price {
    usd: f64
}

impl CoingeckoPriceClient {
    pub fn new(configuration: &CoingeckoPriceClientConfiguration) -> Self {
        let mut headers = HeaderMap::new();
        if let Some(ref api_key) = configuration.api_key {
            headers.insert(HeaderName::from_str("x-cg-pro-api-key").unwrap(), HeaderValue::from_str(api_key).unwrap());
        }

        Self {
            endpoint: configuration.endpoint.to_string(),
            client: ClientBuilder::new().default_headers(headers).build().expect("invalid client"),

            cache: ExpirableCache::new(128)
        }
    }

    pub async fn fetch_token(&self, token: &Felt) -> Result<TokenPrice, Error> {
        let strk_price = self.fetch_token_by_address(&Token::STRK).await?;
        if !strk_price.usd.is_normal() {
            return Err(Error::InvalidPrice(*token))
        }

        let token_price = self.fetch_token_by_address(token).await?;

        Ok(TokenPrice {
            address: *token,
            price_in_strk: normalize_felt(token_price.usd / strk_price.usd, 18)
        })
    }

    async fn fetch_token_by_address(&self, token: &Felt) -> Result<Price, Error> {
        if let Some(price) = self.fetch_token_from_cache(token) {
            return Ok(price)
        }

        self.fetch_token_from_coingecko(token).await
    }

    fn fetch_token_from_cache(&self, token: &Felt) -> Option<Price> {
        self.cache.get_if_not_expired(token)
    }

    async fn fetch_token_from_coingecko(&self, token: &Felt) -> Result<Price, Error> {
        let tokens = [token]
            .map(|x| x.to_hex_string())
            .join(",");

        let mut url = Url::parse(&self.endpoint)
            .and_then(|x| x.join("/api/v3/simple/token_price/starknet"))
            .map_err(|x| Error::URL(x.to_string()))?;

        url
            .query_pairs_mut()
            .append_pair("contract_addresses", &tokens)
            .append_pair("vs_currencies", "usd");

        let response: PriceResponse = self
            .client
            .get(url)
            .send()
            .await?
            .json()
            .await?;

        let price = response
            .0
            .get(token)
            .cloned()
            .ok_or(Error::InvalidPrice(*token))?;

        self.cache.insert(*token, price, Duration::from_secs(3));
        Ok(price)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn fetch_tokens_works_properly() {

    }
}