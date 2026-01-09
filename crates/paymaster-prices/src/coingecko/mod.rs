use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use crate::decimals::DecimalsResolver;
use crate::{Error, PriceClient, PriceOracleConfiguration, TokenPrice};
use paymaster_common::cache::ExpirableCache;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::normalize_felt;
use paymaster_starknet::Configuration as StarknetConfiguration;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest::{ClientBuilder, Url};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use starknet::macros::felt_hex;

pub const DEFAULT_COINGECKO_PRICE_ENDPOINT: &str = "https://api.coingecko.com";

pub const DEFAULT_COINGECKO_SEPOLIA_TOKENS: [(Felt, &str); 3] = [
    (felt_hex!("0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7"), "ethereum"), // EHT
    (felt_hex!("0x512feac6339ff7889822cb5aa2a86c848e9d392bb0e3e237c008674feed8343"), "usd-coin"),  // USDC
    (felt_hex!("0x30de54c07e57818ae4a1210f2a3018a0b9521b8f8ae5206605684741650ac25"), "wrapped-steth"), // wstETH
];

pub const DEFAULT_COINGECKO_MAINNET_TOKENS: [(Felt, &str); 5] = [
    (felt_hex!("0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7"), "ethereum"), // EHT
    (felt_hex!("0x033068F6539f8e6e6b131e6B2B814e6c34A5224bC66947c47DaB9dFeE93b35fb"), "usd-coin"), // USDC
    (felt_hex!("0x068F5c6a61780768455de69077E07e89787839bf8166dEcfBf92B645209c0fB8"), "tether"),   // USDT
    (felt_hex!("0x03Fe2b97C1Fd336E750087D68B9b867997Fd64a2661fF3ca5A7C771641e8e7AC"), "wrapped-bitcoin"), // WBTC
    (felt_hex!("0x0057912720381Af14B0E5C87aa4718ED5E527eaB60B3801ebF702AB09139E38b"), "wrapped-steth"), // wstETH
];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoingeckoPriceClientConfiguration {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub address_to_id: HashMap<Felt, String>,

    pub starknet: StarknetConfiguration,
}

impl From<CoingeckoPriceClientConfiguration> for PriceOracleConfiguration {
    fn from(value: CoingeckoPriceClientConfiguration) -> Self {
        Self::Coingecko(value)
    }
}

#[derive(Clone)]
pub struct CoingeckoPriceClient {
    endpoint: String,
    client: reqwest::Client,

    address_to_id: HashMap<Felt, String>,

    resolver: DecimalsResolver,
    cache: ExpirableCache<Felt, Price>,
}

impl From<CoingeckoPriceClient> for PriceClient {
    fn from(value: CoingeckoPriceClient) -> Self {
        Self::Coingecko(value)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CoingeckoResponse<T> {
    Success(T),
    Error { status: ErrorResponse },
}

#[derive(Deserialize)]
struct ErrorResponse {
    error_code: u64,
    error_message: String,
}

#[derive(Debug, Deserialize)]
struct PriceResponse(HashMap<String, Price>);

#[derive(Deserialize, Debug, Clone, Copy)]
struct Price {
    usd: f64,
}

impl CoingeckoPriceClient {
    pub fn new(configuration: &CoingeckoPriceClientConfiguration) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str("starknet-paymaster").unwrap());

        if let Some(ref api_key) = configuration.api_key {
            headers.insert(HeaderName::from_str("x-cg-pro-api-key").unwrap(), HeaderValue::from_str(api_key).unwrap());
        }

        let mut address_to_id = configuration.address_to_id.clone();
        address_to_id.insert(Token::STRK_ADDRESS, "starknet".to_string());

        Self {
            endpoint: configuration.endpoint.to_string(),
            client: ClientBuilder::new().default_headers(headers).build().expect("invalid client"),

            address_to_id,

            resolver: DecimalsResolver::new(&configuration.starknet),
            cache: ExpirableCache::new(128),
        }
    }

    pub async fn fetch_token(&self, token: &Felt) -> Result<TokenPrice, Error> {
        let strk_price = self.fetch_token_by_address(&Token::STRK_ADDRESS).await?;
        if !strk_price.usd.is_normal() {
            return Err(Error::InvalidPrice(*token));
        }

        let token_price = self.fetch_token_by_address(token).await?;
        let decimals = self.resolver.resolve_decimals(token).await?;

        Ok(TokenPrice {
            address: *token,
            decimals,
            price_in_strk: normalize_felt(token_price.usd / strk_price.usd, 18),
        })
    }

    async fn fetch_token_by_address(&self, token: &Felt) -> Result<Price, Error> {
        if let Some(price) = self.fetch_token_from_cache(token) {
            return Ok(price);
        }

        self.fetch_token_from_coingecko(token).await
    }

    fn fetch_token_from_cache(&self, token: &Felt) -> Option<Price> {
        self.cache.get_if_not_expired(token)
    }

    async fn fetch_token_from_coingecko(&self, token: &Felt) -> Result<Price, Error> {
        let token_id = self
            .address_to_id
            .get(token)
            .ok_or(Error::Internal(format!("unknown token {:?}", token.to_hex_string())))?;

        let mut url = Url::parse(&self.endpoint)
            .and_then(|x| x.join("/api/v3/simple/price"))
            .map_err(|x| Error::URL(x.to_string()))?;

        url.query_pairs_mut()
            .append_pair("ids", token_id)
            .append_pair("vs_currencies", "usd");

        let response: CoingeckoResponse<PriceResponse> = self.client.get(url).send().await?.json().await?;

        let prices = match response {
            CoingeckoResponse::Success(x) => x,
            CoingeckoResponse::Error { status } => return Err(Error::Internal(status.error_message)),
        };

        let price = prices.0.get(token_id).cloned().ok_or(Error::InvalidPrice(*token))?;

        self.cache.insert(*token, price, Duration::from_secs(3));
        Ok(price)
    }
}

#[cfg(test)]
mod tests {
    use crate::coingecko::{CoingeckoPriceClient, CoingeckoPriceClientConfiguration, DEFAULT_COINGECKO_MAINNET_TOKENS, DEFAULT_COINGECKO_PRICE_ENDPOINT};
    use paymaster_starknet::{ChainID, DEFAULT_MAINNET_RPC_ENDPOINT};
    use starknet::core::types::Felt;

    #[ignore] // We get rate limited otherwise
    #[tokio::test]
    async fn should_return_tokens() {
        // Given
        let oracle = CoingeckoPriceClient::new(&CoingeckoPriceClientConfiguration {
            endpoint: DEFAULT_COINGECKO_PRICE_ENDPOINT.to_string(),
            api_key: None,

            address_to_id: DEFAULT_COINGECKO_MAINNET_TOKENS
                .into_iter()
                .map(|(x, y)| (x, y.to_string()))
                .collect(),

            starknet: paymaster_starknet::Configuration {
                endpoint: DEFAULT_MAINNET_RPC_ENDPOINT.to_string(),
                chain_id: ChainID::Mainnet,
                timeout: 10,
                fallbacks: vec![],
            },
        });

        for (token, _) in DEFAULT_COINGECKO_MAINNET_TOKENS {
            // When
            let result = oracle.fetch_token(&token).await.unwrap();

            // Then
            assert_eq!(result.address, token);
            assert!(result.price_in_strk > Felt::ZERO);
        }
    }
}
