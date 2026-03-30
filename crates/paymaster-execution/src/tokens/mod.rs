//! Token service for fetching and caching token metadata.
//!
//! This module provides a service that fetches token information from the AVNU API
//! and caches it locally with a 1-hour TTL using SyncValue.

use std::collections::HashMap;
use std::time::Duration;

use paymaster_common::concurrency::SyncValue;
use paymaster_starknet::math::denormalize_felt;
use paymaster_starknet::ChainID;
use serde::Deserialize;
use starknet::core::types::Felt;
use thiserror::Error;
use tracing::warn;

/// Base URL for the AVNU API on mainnet.
const AVNU_API_MAINNET_URL: &str = "https://starknet.api.avnu.fi";

/// Base URL for the AVNU API on Sepolia testnet.
const AVNU_API_SEPOLIA_URL: &str = "https://sepolia.api.avnu.fi";

/// Cache TTL: 1 hour.
const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Token information from the AVNU API.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub name: String,
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub logo_uri: Option<String>,
}

/// Paginated response from the tokens API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageToken {
    content: Vec<TokenInfo>,
    total_pages: u32,
}

/// Errors that can occur when fetching tokens.
#[derive(Debug, Error, Clone)]
pub enum TokenServiceError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

/// Token cache type alias.
type Tokens = HashMap<Felt, TokenInfo>;

/// Token service that caches token metadata.
///
/// Uses SyncValue with a 1-hour TTL for automatic cache refresh.
#[derive(Clone)]
pub struct TokenClient {
    /// Cached tokens indexed by address, with automatic TTL-based refresh.
    cache: SyncValue<Tokens>,
    /// HTTP client
    client: reqwest::Client,
    /// Base URL for the API
    base_url: String,
}

impl TokenClient {
    /// Creates a new token service for mainnet.
    pub fn mainnet() -> Self {
        Self::with_base_url(AVNU_API_MAINNET_URL)
    }

    /// Creates a new token service for Sepolia testnet.
    pub fn sepolia() -> Self {
        Self::with_base_url(AVNU_API_SEPOLIA_URL)
    }

    /// Creates a new token service based on chain ID.
    pub fn new(chain_id: ChainID) -> Self {
        match chain_id {
            ChainID::Sepolia => Self::sepolia(),
            ChainID::Mainnet => Self::mainnet(),
        }
    }

    fn with_base_url(base_url: &str) -> Self {
        Self {
            cache: SyncValue::new(CACHE_TTL),
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Gets token info by address.
    ///
    /// Automatically refreshes the cache if it has expired (1-hour TTL).
    /// Returns `None` if the token is not found.
    pub async fn get_token(&self, address: Felt) -> Option<TokenInfo> {
        let cache = self
            .cache
            .read_or_refresh({
                let this = self.clone();
                move || Box::pin(async move { this.fetch_all_tokens().await })
            })
            .await
            .ok()?;
        cache.get(&address).cloned()
    }

    async fn fetch_token_page(&self, page: u32, page_size: u32) -> Result<PageToken, TokenServiceError> {
        let url = format!("{}/v1/starknet/tokens?page={}&size={}", self.base_url, page, page_size);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| TokenServiceError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(TokenServiceError::HttpError(format!("API returned status {}", response.status())));
        }

        response.json().await.map_err(|e| TokenServiceError::ParseError(e.to_string()))
    }

    /// Fetches all tokens from the API.
    async fn fetch_all_tokens(&self) -> Result<Tokens, TokenServiceError> {
        let page_size = 1000;

        let first_page = self.fetch_token_page(0, page_size).await?;
        let total_pages = first_page.total_pages;

        let mut all_tokens = first_page.content;

        for page in 1..total_pages {
            let page_response = self.fetch_token_page(page, page_size).await?;
            all_tokens.extend(page_response.content);
        }

        // Build the cache
        let mut cache = HashMap::new();
        for token in all_tokens {
            // Parse address to Felt for consistent lookup (API always returns hex format)
            if let Ok(felt) = Felt::from_hex(&token.address) {
                cache.insert(felt, token);
            } else {
                warn!("Failed to parse token address: {}", token.address);
            }
        }

        Ok(cache)
    }

    /// Normalizes a token amount using the token's decimals.
    ///
    /// For example, with 18 decimals:
    /// - 1000000000000000000 (1e18) becomes 1.0
    /// - 100000000000000000 (1e17) becomes 0.1
    pub async fn try_normalize_amount(&self, address: Felt, raw_amount: Felt) -> Option<f64> {
        let token = self.get_token(address).await?;
        Some(denormalize_felt(raw_amount, token.decimals as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paymaster_starknet::constants::Token;

    mod new {
        use super::*;

        #[test]
        fn should_use_mainnet_url_for_mainnet_chain() {
            let client = TokenClient::new(ChainID::Mainnet);
            assert_eq!(client.base_url, AVNU_API_MAINNET_URL);
        }

        #[test]
        fn should_use_sepolia_url_for_sepolia_chain() {
            let client = TokenClient::new(ChainID::Sepolia);
            assert_eq!(client.base_url, AVNU_API_SEPOLIA_URL);
        }
    }

    mod get_token {
        use super::*;

        #[tokio::test]
        async fn should_fetch_and_parse_token_info() {
            let client = TokenClient::mainnet();
            let token = client.get_token(Token::ETH_ADDRESS).await.expect("ETH should exist");

            assert_eq!(token.symbol, "ETH");
            assert_eq!(token.decimals, 18);
        }

        #[tokio::test]
        async fn should_return_none_for_unknown_token() {
            let client = TokenClient::mainnet();
            let result = client.get_token(Felt::from(0x123u64)).await;
            assert!(result.is_none());
        }
    }
}
