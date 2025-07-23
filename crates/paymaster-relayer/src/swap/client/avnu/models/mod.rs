use paymaster_common::service::Error as ServiceError;
use serde::Deserialize;
use starknet::core::types::{Call, Felt};
use starknet::core::utils::get_selector_from_name;

// Quote Response Returned By AVNU Client
#[derive(Debug, Deserialize, Clone)]
pub struct AVNUQuote {
    #[serde(rename = "quoteId")]
    pub quote_id: String,
    #[serde(rename = "sellAmount")]
    pub sell_amount: String,
    #[serde(rename = "buyAmount")]
    pub buy_amount: String,
    #[serde(rename = "sellAmountInUsd")]
    pub sell_amount_in_usd: Option<f64>,
    #[serde(rename = "buyAmountInUsd")]
    pub buy_amount_in_usd: Option<f64>,
}

impl AVNUQuote {
    // Validates the quote price by comparing USD values
    pub fn assert_security(&self, max_price_impact: f64) -> Result<(), ServiceError> {
        let sell_amount_in_usd = self
            .sell_amount_in_usd
            .ok_or_else(|| ServiceError::new("Missing USD value for sell amount in quote"))?;

        let buy_amount_in_usd = self
            .buy_amount_in_usd
            .ok_or_else(|| ServiceError::new("Missing USD value for buy amount in quote"))?;

        if sell_amount_in_usd <= 0.0 || buy_amount_in_usd <= 0.0 {
            return Err(ServiceError::new("Invalid USD values in quote"));
        }

        let price_impact = (sell_amount_in_usd - buy_amount_in_usd) / sell_amount_in_usd;

        if price_impact > max_price_impact.abs() {
            return Err(ServiceError::new(&format!(
                "Quote price impact is too high: {:.2}% (max allowed: {:.2}%)",
                price_impact * 100.0,
                max_price_impact.abs() * 100.0
            )));
        }
        Ok(())
    }

    // Verify that the quote is above the minimum sell value
    pub fn assert_min_sell_value(&self, min_usd_sell_amount: f64) -> Result<Self, ServiceError> {
        if self.sell_amount_in_usd.unwrap_or(0.0) < min_usd_sell_amount {
            return Err(ServiceError::new(&format!(
                "Sell amount in USD is below the minimum sell value: {:.2} USD (min: {:.2} USD)",
                self.sell_amount_in_usd.unwrap_or(0.0),
                min_usd_sell_amount
            )));
        }
        Ok(self.clone())
    }

    // Get the minimum amount of tokens received after applying slippage
    pub fn get_min_received(&self, slippage: f64) -> Result<Felt, ServiceError> {
        // Parse the hex string to Felt first
        let buy_amount_felt = Felt::from_hex(&self.buy_amount).map_err(|e| ServiceError::new(&format!("Failed to parse buy amount hex '{}': {}", self.buy_amount, e)))?;

        // Convert to u128 for calculation
        let buy_amount_u128: u128 = buy_amount_felt
            .try_into()
            .map_err(|e| ServiceError::new(&format!("Failed to convert buy amount to u128: {}", e)))?;

        // Apply slippage
        let min_received_u128 = ((buy_amount_u128 as f64) * (1.0 - slippage)) as u128;

        // Convert back to Felt
        Ok(Felt::from(min_received_u128))
    }
}

#[derive(Debug, Deserialize)]
pub struct AVNUCall {
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub entrypoint: String,
    pub calldata: Vec<String>,
}

impl AVNUCall {
    pub fn as_call(&self) -> Call {
        Call {
            to: Felt::from_hex(&self.contract_address).unwrap(),
            selector: get_selector_from_name(&self.entrypoint).unwrap(),
            calldata: self.calldata.iter().map(|s| Felt::from_hex(s).unwrap()).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AVNUBuildedQuote {
    #[serde(rename = "chainId")]
    pub chain_id: String,
    pub calls: Vec<AVNUCall>,
}
