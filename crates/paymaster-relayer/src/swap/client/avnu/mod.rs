pub mod models;

use std::time::Duration;

use crate::swap::client::avnu::models::{AVNUBuildedQuote, AVNUQuote};
use crate::swap::client::{Swap, SwapClientConfiguration};
use crate::swap::SwapClient;
use async_trait::async_trait;
use paymaster_common::service::Error as ServiceError;
use reqwest::Client as HTTPClient;
use serde_json::json;
use starknet::core::types::{Call, Felt};

pub const DEFAULT_SEPOLIA_AVNU_SWAP_ENDPOINT: &str = "https://sepolia.api.avnu.fi/swap/v3";
pub const DEFAULT_MAINNET_AVNU_SWAP_ENDPOINT: &str = "https://starknet.api.avnu.fi/swap/v3";

#[derive(Clone)]
pub struct AVNUSwapClient {
    endpoint: String,
    client: HTTPClient,
}

impl From<AVNUSwapClient> for SwapClient {
    fn from(value: AVNUSwapClient) -> Self {
        Self::AVNU(value)
    }
}

impl AVNUSwapClient {
    pub fn new(configuration: &SwapClientConfiguration) -> Self {
        Self {
            endpoint: configuration.endpoint.clone(),
            client: HTTPClient::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .expect("invalid client"),
        }
    }

    // Get quotes fora swap
    async fn get_quote(&self, sell_token: Felt, buy_token: Felt, sell_amount: Felt, taker_address: Felt, max_price_impact: f64) -> Result<AVNUQuote, ServiceError> {
        let response = self
            .client
            .get(&format!("{}/quotes", self.endpoint))
            .query(&[
                ("sellTokenAddress", &format!("0x{:x}", sell_token)),
                ("buyTokenAddress", &format!("0x{:x}", buy_token)),
                ("sellAmount", &format!("0x{:x}", sell_amount)),
                ("takerAddress", &format!("0x{:x}", taker_address)),
            ])
            .send()
            .await
            .map_err(|e| ServiceError::new(&format!("Failed to get AVNU quotes: {}", e)))?;

        let response = response
            .error_for_status()
            .map_err(|e| ServiceError::new(&format!("AVNU Quotes API returned error: {}", e)))?;

        let quotes: Vec<AVNUQuote> = response
            .json()
            .await
            .map_err(|e| ServiceError::new(&format!("Failed to parse AVNU quote response: {}", e)))?;
        // Use best quote
        if quotes.is_empty() {
            return Err(ServiceError::new("No quotes returned by AVNU"));
        }
        let quote = quotes.into_iter().next().unwrap();

        // Verify security of the quote
        quote.assert_security(max_price_impact)?;

        Ok(quote)
    }

    // Build transaction calls based on quote_id received
    async fn build_transaction(&self, quote_id: &str, taker_address: Felt, slippage: f64) -> Result<AVNUBuildedQuote, ServiceError> {
        let request_body = json!({
            "quoteId": quote_id,
            "takerAddress": format!("0x{:x}", taker_address),
            "slippage": slippage,
            "includeApprove": true
        });

        let response = self
            .client
            .post(&format!("{}/build", self.endpoint))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ServiceError::new(&format!("Failed to build transaction through AVNU: {}", e)))?;

        let response = response
            .error_for_status()
            .map_err(|e| ServiceError::new(&format!("AVNU Build API returned error: {}", e)))?;

        let response: AVNUBuildedQuote = response
            .json()
            .await
            .map_err(|e| ServiceError::new(&format!("Failed to parse AVNU build response: {}", e)))?;

        Ok(response)
    }
}

// Implementation of Swap trait for AVNU swap client
#[async_trait]
impl Swap for AVNUSwapClient {
    async fn swap(
        &self,
        sell_token: Felt,
        buy_token: Felt,
        sell_amount: Felt,
        taker_address: Felt,
        slippage: f64,
        max_price_impact: f64,
        min_usd_sell_amount: f64,
    ) -> Result<(Vec<Call>, Felt), ServiceError> {
        // Get quote
        let quote = self
            .get_quote(sell_token, buy_token, sell_amount, taker_address, max_price_impact)
            .await?;

        quote.assert_min_sell_value(min_usd_sell_amount)?;
        // Get the minimum amount of tokens we are guaranteed to receive
        let min_received = quote.get_min_received(slippage)?;

        // Build transaction
        let build_response = self.build_transaction(&quote.quote_id, taker_address, slippage).await?;
        let calls = build_response.calls.into_iter().map(|call| call.as_call()).collect();
        Ok((calls, min_received))
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn should_return_tokens() {}
}
