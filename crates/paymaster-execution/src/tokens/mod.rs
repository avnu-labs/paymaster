//! Token service for fetching and caching token metadata.
//!
//! This module provides a service that fetches token information from the AVNU API
//! and caches it locally with a 1-hour TTL using SyncValue.

use std::collections::HashMap;
use std::time::Duration;

use paymaster_common::concurrency::SyncValue;
use serde::Deserialize;
use starknet::core::types::Felt;
use thiserror::Error;
use tracing::{debug, warn};

/// Base URL for the AVNU API on mainnet.
const AVNU_API_MAINNET_URL: &str = "https://starknet.api.avnu.fi";

/// Base URL for the AVNU API on Sepolia testnet.
const AVNU_API_SEPOLIA_URL: &str = "https://sepolia.api.avnu.fi";

/// Starknet Sepolia chain ID.
const CHAIN_ID_SEPOLIA: Felt = Felt::from_hex_unchecked("0x534e5f5345504f4c4941");

/// Cache TTL: 1 hour.
const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Token information from the AVNU API.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    /// Token name (e.g., "Ethereum")
    #[serde(default)]
    pub name: String,
    /// Token contract address
    #[serde(default)]
    pub address: String,
    /// Token symbol (e.g., "ETH")
    #[serde(default)]
    pub symbol: String,
    /// Number of decimals
    #[serde(default)]
    pub decimals: u8,
    /// Logo URI (optional)
    #[serde(default)]
    pub logo_uri: Option<String>,
}

/// Paginated response from the tokens API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageTokenDto {
    content: Vec<TokenInfo>,
    total_pages: u32,
    number: u32,
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
type TokenCache = HashMap<Felt, TokenInfo>;

/// Token service that caches token metadata.
///
/// Uses SyncValue with a 1-hour TTL for automatic cache refresh.
#[derive(Clone)]
pub struct TokenService {
    /// Cached tokens indexed by address, with automatic TTL-based refresh.
    cache: SyncValue<TokenCache>,
    /// HTTP client
    client: reqwest::Client,
    /// Base URL for the API
    base_url: String,
}

impl TokenService {
    /// Creates a new token service for mainnet.
    pub fn mainnet() -> Self {
        Self::with_base_url(AVNU_API_MAINNET_URL)
    }

    /// Creates a new token service for Sepolia testnet.
    pub fn sepolia() -> Self {
        Self::with_base_url(AVNU_API_SEPOLIA_URL)
    }

    /// Creates a new token service based on chain ID.
    pub fn for_chain(chain_id: Felt) -> Self {
        if chain_id == CHAIN_ID_SEPOLIA {
            Self::sepolia()
        } else {
            // Default to mainnet for unknown chain IDs
            Self::mainnet()
        }
    }

    /// Creates a new token service with a custom base URL (useful for testing).
    pub fn with_base_url(base_url: &str) -> Self {
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
    pub async fn get(&self, address: Felt) -> Option<TokenInfo> {
        let cache = self.get_cache().await.ok()?;
        cache.get(&address).cloned()
    }

    /// Gets the token cache, refreshing if stale.
    async fn get_cache(&self) -> Result<TokenCache, TokenServiceError> {
        self.cache
            .read_or_refresh({
                let this = self.clone();
                move || Box::pin(async move { this.fetch_all_tokens().await })
            })
            .await
    }

    /// Fetches all tokens from the API.
    async fn fetch_all_tokens(&self) -> Result<TokenCache, TokenServiceError> {
        let mut all_tokens = Vec::new();
        let mut page = 0;
        let page_size = 1000;

        loop {
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

            let page_response: PageTokenDto = response
                .json()
                .await
                .map_err(|e| TokenServiceError::ParseError(e.to_string()))?;

            all_tokens.extend(page_response.content);

            // Check if we've fetched all pages
            if page_response.number + 1 >= page_response.total_pages {
                break;
            }

            page += 1;
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

        debug!("Token cache refreshed with {} tokens", cache.len());
        Ok(cache)
    }

    /// Normalizes a token amount using the token's decimals.
    ///
    /// For example, with 18 decimals:
    /// - 1000000000000000000 (1e18) becomes 1.0
    /// - 100000000000000000 (1e17) becomes 0.1
    pub async fn normalize_amount(&self, address: Felt, raw_amount: u128) -> Option<f64> {
        let token = self.get(address).await?;
        let divisor = 10u128.pow(token.decimals as u32);
        Some(raw_amount as f64 / divisor as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod token_service {
        use super::*;

        #[test]
        fn should_create_mainnet_service_for_mainnet_chain_id() {
            // Given - mainnet chain ID
            let chain_id = Felt::from_hex_unchecked("0x534e5f4d41494e");

            // When
            let service = TokenService::for_chain(chain_id);

            // Then
            assert_eq!(service.base_url, AVNU_API_MAINNET_URL);
        }

        #[test]
        fn should_create_sepolia_service_for_sepolia_chain_id() {
            // Given
            let chain_id = CHAIN_ID_SEPOLIA;

            // When
            let service = TokenService::for_chain(chain_id);

            // Then
            assert_eq!(service.base_url, AVNU_API_SEPOLIA_URL);
        }

        #[tokio::test]
        async fn should_return_none_for_unknown_token() {
            // Given
            let service = TokenService::mainnet();
            let unknown_address = Felt::from(0x123u64);

            // When - this will refresh cache and then look for the unknown token
            let result = service.get(unknown_address).await;

            // Then
            assert!(result.is_none());
        }
    }

    mod normalize_amount {
        use super::*;

        #[tokio::test]
        async fn should_return_none_for_unknown_token() {
            // Given
            let service = TokenService::mainnet();
            let unknown_address = Felt::from(0x123u64);

            // When - this will refresh the cache and then look for the unknown token
            let result = service.normalize_amount(unknown_address, 1000000000000000000).await;

            // Then
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn should_normalize_eth_amount_after_refresh() {
            // Given
            let service = TokenService::mainnet();
            let eth_address = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
            let raw_amount: u128 = 100_000_000_000_000_000; // 0.1 ETH in wei (18 decimals)

            // When - normalize will automatically refresh cache if empty
            let normalized = service.normalize_amount(eth_address, raw_amount).await;
            assert!(normalized.is_some(), "ETH token not found in cache");
            let value = normalized.unwrap();
            assert!((value - 0.1).abs() < 0.0001, "Expected 0.1, got {}", value);

            // Verify token info
            let token = service.get(eth_address).await;
            assert!(token.is_some());
            let token = token.unwrap();
            assert_eq!(token.symbol, "ETH");
            assert_eq!(token.decimals, 18);
        }
    }

    mod e2e {
        use super::*;

        #[tokio::test]
        async fn should_fetch_tokens_from_mainnet_api() {
            // Given
            let service = TokenService::mainnet();
            let eth_address = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

            // When - get will trigger a refresh
            let result = service.get(eth_address).await;

            // Then
            assert!(result.is_some(), "Should have fetched ETH token");
        }

        #[tokio::test]
        async fn should_fetch_tokens_from_sepolia_api() {
            // Given
            let service = TokenService::sepolia();
            let eth_address = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

            // When - get will trigger a refresh
            let result = service.get(eth_address).await;

            // Then - Sepolia should also have ETH
            assert!(result.is_some(), "Should have fetched ETH token on Sepolia");
        }

        #[tokio::test]
        async fn should_find_common_tokens() {
            // Given
            let service = TokenService::mainnet();
            let eth_address = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
            let strk_address = Felt::from_hex_unchecked("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

            // When - get() will automatically refresh if cache is empty
            let eth = service.get(eth_address).await;
            assert!(eth.is_some(), "ETH should be in token list");
            let eth = eth.unwrap();
            assert_eq!(eth.symbol, "ETH");
            assert_eq!(eth.decimals, 18);

            let strk = service.get(strk_address).await;
            assert!(strk.is_some(), "STRK should be in token list");
            let strk = strk.unwrap();
            assert_eq!(strk.symbol, "STRK");
            assert_eq!(strk.decimals, 18);
        }

        #[tokio::test]
        async fn should_use_cached_value_on_second_call() {
            // Given
            let service = TokenService::mainnet();
            let eth_address = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

            // When - first call triggers refresh
            let first = service.get(eth_address).await;
            assert!(first.is_some(), "First call should return ETH");

            // When - second call should use cache (no network call)
            let second = service.get(eth_address).await;
            assert!(second.is_some(), "Second call should return ETH from cache");

            // Then - both should return the same data
            assert_eq!(first.unwrap().symbol, second.unwrap().symbol);
        }
    }
}
