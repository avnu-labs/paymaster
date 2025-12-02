//! AVNU Exchange contract metadata extractor.
//!
//! Extracts diagnostic information from failed AVNU swap transactions,
//! particularly focusing on slippage errors and swap parameters.

use async_trait::async_trait;
use starknet::core::types::{Call, Felt};
use std::collections::HashMap;

use crate::diagnostics::{CallDiagnostic, CallMetadataExtractor, DiagnosticContext, DiagnosticMetric, DiagnosticValue, ErrorCategory};
use crate::tokens::TokenService;

/// AVNU Exchange contract address on Starknet mainnet.
pub const AVNU_EXCHANGE_ADDRESS: Felt = Felt::from_hex_unchecked("0x04270219d365d6b017231b52e92b3fb5d7c8378b05e9abc97724537a80e93b0f");

/// Converts a Felt to u128, returning 0 if the value doesn't fit.
fn felt_to_u128(felt: Felt) -> u128 {
    felt.to_biguint().try_into().unwrap_or(0)
}

/// Known error messages from the AVNU Exchange contract.
mod errors {
    /// Slippage exceeded - buy_token_min_amount > buy_token_final_amount
    pub const INSUFFICIENT_TOKENS_RECEIVED: &str = "insufficient tokens received";

    /// Slippage exceeded - sell_token_max_amount < sell_token_amount in swap_exact_token_to
    pub const INVALID_TOKEN_MAX_AMOUNT: &str = "invalid token from max amount";

    /// User doesn't have enough tokens to sell
    pub const TOKEN_BALANCE_TOO_LOW: &str = "token from balance is too low";

    /// Token amount is zero
    pub const TOKEN_AMOUNT_ZERO: &str = "token from amount is 0";

    /// Routes array is empty
    pub const ROUTES_EMPTY: &str = "routes is empty";

    /// First route sell token doesn't match
    pub const INVALID_TOKEN_FROM: &str = "invalid token from";

    /// Last route buy token doesn't match
    pub const INVALID_TOKEN_TO: &str = "invalid token to";

    /// Unknown exchange in routes
    pub const UNKNOWN_EXCHANGE: &str = "unknown exchange";
}

/// Selectors for AVNU Exchange functions.
mod selectors {
    use starknet::core::types::Felt;
    use starknet::macros::selector;

    pub fn multi_route_swap() -> Felt {
        selector!("multi_route_swap")
    }

    pub fn swap_exact_token_to() -> Felt {
        selector!("swap_exact_token_to")
    }

    pub fn swap_external_solver() -> Felt {
        selector!("swap_external_solver")
    }
}

/// Extractor for AVNU Exchange contract errors.
///
/// Handles the following swap functions:
/// - `multi_route_swap`: Standard swap with slippage protection
/// - `swap_exact_token_to`: Swap to receive exact amount of buy token
/// - `swap_external_solver`: Swap using external solver
///
/// # Extracted Metadata
///
/// For swap errors, extracts:
/// - `sell_token`: Token being sold (address)
/// - `sell_token_symbol`: Token symbol (e.g., "ETH") - when available
/// - `sell_token_name`: Token name (e.g., "Ethereum") - when available
/// - `sell_amount`: Normalized amount of sell token - when token info available
/// - `sell_amount_hex`: Raw amount as hex string (always present)
/// - `buy_token`: Token being bought (address)
/// - `buy_token_symbol`: Token symbol - when available
/// - `buy_token_name`: Token name - when available
/// - `buy_amount`: Normalized amount - when token info available
/// - `buy_amount_hex`: Raw amount as hex string (always present)
/// - `buy_min_amount` / `buy_min_amount_hex`: Minimum expected (for slippage)
/// - `beneficiary`: Recipient of the swap
#[derive(Clone)]
pub struct AvnuExtractor {
    /// The AVNU Exchange contract address
    contract_address: Felt,
    /// Optional token service for enriching metadata
    token_service: Option<TokenService>,
}

impl AvnuExtractor {
    /// Creates a new AVNU extractor for the given contract address.
    pub fn new(contract_address: Felt) -> Self {
        Self {
            contract_address,
            token_service: None,
        }
    }

    /// Creates a new AVNU extractor with a token service for enriched metadata.
    pub fn with_token_service(contract_address: Felt, token_service: TokenService) -> Self {
        Self {
            contract_address,
            token_service: Some(token_service),
        }
    }

    /// Checks if the error message indicates a slippage error.
    fn is_slippage_error(&self, error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(errors::INSUFFICIENT_TOKENS_RECEIVED) || lower.contains(errors::INVALID_TOKEN_MAX_AMOUNT)
    }

    /// Checks if the error message indicates a balance error.
    fn is_balance_error(&self, error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(errors::TOKEN_BALANCE_TOO_LOW) || lower.contains(errors::TOKEN_AMOUNT_ZERO)
    }

    /// Checks if the error message indicates an input validation error.
    fn is_input_error(&self, error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(errors::ROUTES_EMPTY) || lower.contains(errors::INVALID_TOKEN_FROM) || lower.contains(errors::INVALID_TOKEN_TO)
    }

    /// Determines the error category from the error message.
    fn categorize_error(&self, error: &str) -> ErrorCategory {
        if self.is_slippage_error(error) {
            ErrorCategory::Slippage
        } else if self.is_balance_error(error) {
            ErrorCategory::InsufficientBalance
        } else if self.is_input_error(error) {
            ErrorCategory::InvalidInput
        } else if error.to_lowercase().contains(errors::UNKNOWN_EXCHANGE) {
            ErrorCategory::RouteNotFound
        } else {
            ErrorCategory::ContractError("avnu".to_string())
        }
    }

    /// Calculates slippage percentage from u256 low parts (max_amount, min_amount).
    ///
    /// Formula: ((max - min) / max) * 100
    ///
    /// Only uses the low part of u256 values since actual amounts never exceed u128.
    /// Returns None if calculation would divide by zero.
    fn calculate_slippage_percent(max_low: u128, min_low: u128) -> Option<f64> {
        if max_low == 0 {
            return None;
        }

        if min_low > max_low {
            return None;
        }

        let diff = max_low - min_low;
        // Calculate percentage with 2 decimal precision: (diff * 10000) / max / 100
        let pct_scaled = diff.checked_mul(10000)?.checked_div(max_low)?;
        let pct = pct_scaled as f64 / 100.0;

        Some(pct)
    }

    /// Enriches metadata with token symbol and name.
    ///
    /// When token info is available, adds:
    /// - `{prefix}_symbol`: Token symbol (e.g., "ETH")
    /// - `{prefix}_name`: Token name (e.g., "Ethereum")
    async fn enrich_token_info(&self, metadata: &mut HashMap<String, DiagnosticValue>, token_address: Felt, prefix: &str) {
        if let Some(ref token_service) = self.token_service {
            if let Some(token_info) = token_service.get(token_address).await {
                metadata.insert(format!("{prefix}_symbol"), token_info.symbol.clone().into());
                metadata.insert(format!("{prefix}_name"), token_info.name.clone().into());
            }
        }
    }

    /// Adds an amount to metadata, both normalized and raw hex.
    ///
    /// Always adds:
    /// - `{key}_hex`: Raw amount as hex string (e.g., "0xde0b6b3a7640000")
    ///
    /// When token info is available, also adds:
    /// - `{key}`: Normalized amount as Float (e.g., 1.5 instead of 1500000000000000000)
    async fn add_amount(&self, metadata: &mut HashMap<String, DiagnosticValue>, token_address: Felt, raw_amount: u128, key: &str) {
        // Always add raw hex value
        metadata.insert(format!("{key}_hex"), format!("0x{:x}", raw_amount).into());

        // Try to normalize with token decimals
        if let Some(ref token_service) = self.token_service {
            if let Some(token_info) = token_service.get(token_address).await {
                let divisor = 10u128.pow(token_info.decimals as u32);
                let normalized = raw_amount as f64 / divisor as f64;
                metadata.insert(key.to_string(), normalized.into());
            }
        }
    }

    /// Extracts parameters from a multi_route_swap call.
    ///
    /// Calldata layout:
    /// 0: sell_token_address
    /// 1-2: sell_token_amount (u256 = low, high)
    /// 3: buy_token_address
    /// 4-5: buy_token_amount (u256)
    /// 6-7: buy_token_min_amount (u256)
    /// 8: beneficiary
    /// 9: integrator_fee_amount_bps
    /// 10: integrator_fee_recipient
    /// 11+: routes (Array<Route>)
    async fn extract_multi_route_swap_params(&self, call: &Call) -> HashMap<String, DiagnosticValue> {
        let mut metadata = HashMap::new();
        let calldata = &call.calldata;

        if calldata.len() >= 11 {
            let sell_token = calldata[0];
            let sell_amount_raw = felt_to_u128(calldata[1]);
            let buy_token = calldata[3];
            let buy_amount_raw = felt_to_u128(calldata[4]);
            let buy_min_amount_raw = felt_to_u128(calldata[6]);

            // Token addresses
            metadata.insert("sell_token".to_string(), sell_token.into());
            metadata.insert("buy_token".to_string(), buy_token.into());

            // Other parameters
            metadata.insert("beneficiary".to_string(), calldata[8].into());
            metadata.insert("integrator_fee_bps".to_string(), felt_to_u128(calldata[9]).into());
            metadata.insert("integrator_fee_recipient".to_string(), calldata[10].into());

            // Enrich token info (symbol, name) - once per token
            self.enrich_token_info(&mut metadata, sell_token, "sell_token").await;
            self.enrich_token_info(&mut metadata, buy_token, "buy_token").await;

            // Add amounts (normalized + hex)
            self.add_amount(&mut metadata, sell_token, sell_amount_raw, "sell_amount").await;
            self.add_amount(&mut metadata, buy_token, buy_amount_raw, "buy_amount").await;
            self.add_amount(&mut metadata, buy_token, buy_min_amount_raw, "buy_min_amount")
                .await;

            // Calculate slippage percentage: ((buy_amount - buy_min_amount) / buy_amount) * 100
            if let Some(slippage_pct) = Self::calculate_slippage_percent(buy_amount_raw, buy_min_amount_raw) {
                metadata.insert("max_slippage_percent".to_string(), slippage_pct.into());
            }
        }

        metadata
    }

    /// Extracts parameters from a swap_exact_token_to call.
    ///
    /// Calldata layout:
    /// 0: sell_token_address
    /// 1-2: sell_token_amount (u256)
    /// 3-4: sell_token_max_amount (u256)
    /// 5: buy_token_address
    /// 6-7: buy_token_amount (u256)
    /// 8: beneficiary
    /// 9: integrator_fee_amount_bps
    /// 10: integrator_fee_recipient
    /// 11+: routes
    async fn extract_swap_exact_token_to_params(&self, call: &Call) -> HashMap<String, DiagnosticValue> {
        let mut metadata = HashMap::new();
        let calldata = &call.calldata;

        if calldata.len() >= 11 {
            let sell_token = calldata[0];
            let sell_amount_raw = felt_to_u128(calldata[1]);
            let sell_max_amount_raw = felt_to_u128(calldata[3]);
            let buy_token = calldata[5];
            let buy_amount_raw = felt_to_u128(calldata[6]);

            // Token addresses
            metadata.insert("sell_token".to_string(), sell_token.into());
            metadata.insert("buy_token".to_string(), buy_token.into());

            // Other parameters
            metadata.insert("beneficiary".to_string(), calldata[8].into());
            metadata.insert("integrator_fee_bps".to_string(), felt_to_u128(calldata[9]).into());
            metadata.insert("integrator_fee_recipient".to_string(), calldata[10].into());

            // Enrich token info (symbol, name) - once per token
            self.enrich_token_info(&mut metadata, sell_token, "sell_token").await;
            self.enrich_token_info(&mut metadata, buy_token, "buy_token").await;

            // Add amounts (normalized + hex)
            self.add_amount(&mut metadata, sell_token, sell_amount_raw, "sell_amount").await;
            self.add_amount(&mut metadata, sell_token, sell_max_amount_raw, "sell_max_amount")
                .await;
            self.add_amount(&mut metadata, buy_token, buy_amount_raw, "buy_amount").await;

            // Calculate slippage percentage: ((sell_max_amount - sell_amount) / sell_max_amount) * 100
            if let Some(slippage_pct) = Self::calculate_slippage_percent(sell_max_amount_raw, sell_amount_raw) {
                metadata.insert("max_slippage_percent".to_string(), slippage_pct.into());
            }
        }

        metadata
    }

    /// Extracts parameters from a swap_external_solver call.
    ///
    /// Calldata layout:
    /// 0: user_address
    /// 1: sell_token_address
    /// 2: buy_token_address
    /// 3: beneficiary
    /// 4: external_solver_address
    /// 5+: external_solver_adapter_calldata
    fn extract_swap_external_solver_params(&self, call: &Call) -> HashMap<String, DiagnosticValue> {
        let mut metadata = HashMap::new();
        let calldata = &call.calldata;

        if calldata.len() >= 5 {
            metadata.insert("user_address".to_string(), calldata[0].into());
            metadata.insert("sell_token".to_string(), calldata[1].into());
            metadata.insert("buy_token".to_string(), calldata[2].into());
            metadata.insert("beneficiary".to_string(), calldata[3].into());
            metadata.insert("external_solver".to_string(), calldata[4].into());
        }

        metadata
    }

    /// Finds the first AVNU swap call in the context.
    fn find_swap_call<'a>(&self, context: &'a DiagnosticContext) -> Option<&'a Call> {
        context.calls_to(self.contract_address).find(|call| {
            call.selector == selectors::multi_route_swap() || call.selector == selectors::swap_exact_token_to() || call.selector == selectors::swap_external_solver()
        })
    }

    /// Extracts swap parameters based on the function selector.
    async fn extract_swap_params(&self, call: &Call) -> HashMap<String, DiagnosticValue> {
        let mut metadata = if call.selector == selectors::multi_route_swap() {
            self.extract_multi_route_swap_params(call).await
        } else if call.selector == selectors::swap_exact_token_to() {
            self.extract_swap_exact_token_to_params(call).await
        } else if call.selector == selectors::swap_external_solver() {
            self.extract_swap_external_solver_params(call)
        } else {
            HashMap::new()
        };

        // Add the function name for clarity
        let function_name = if call.selector == selectors::multi_route_swap() {
            "multi_route_swap"
        } else if call.selector == selectors::swap_exact_token_to() {
            "swap_exact_token_to"
        } else if call.selector == selectors::swap_external_solver() {
            "swap_external_solver"
        } else {
            "unknown"
        };

        metadata.insert("function".to_string(), function_name.into());
        metadata
    }

    /// Builds AVNU-specific metrics from extracted metadata.
    ///
    /// Emits:
    /// - `avnu_slippage_percent`: Max slippage percentage for slippage errors
    /// - `avnu_sell_amount`: Normalized sell amount for swap errors
    /// - `avnu_buy_amount`: Normalized buy amount for swap errors
    fn build_metrics(&self, metadata: &HashMap<String, DiagnosticValue>, category: &ErrorCategory) -> Vec<DiagnosticMetric> {
        let mut metrics = Vec::new();

        // Get token symbols for labels (if available)
        let sell_token_symbol = Self::get_string_value(metadata, "sell_token_symbol");
        let buy_token_symbol = Self::get_string_value(metadata, "buy_token_symbol");

        // Slippage metric - useful for understanding slippage distribution in errors
        if let Some(DiagnosticValue::Float(slippage)) = metadata.get("max_slippage_percent") {
            let mut metric = DiagnosticMetric::new("avnu_slippage_percent", *slippage);
            if let Some(ref symbol) = sell_token_symbol {
                metric = metric.with_label("sell_token", symbol.clone());
            }
            if let Some(ref symbol) = buy_token_symbol {
                metric = metric.with_label("buy_token", symbol.clone());
            }
            metrics.push(metric);
        }

        // Sell amount metric - useful for understanding which amounts fail
        if let Some(DiagnosticValue::Float(amount)) = metadata.get("sell_amount") {
            let mut metric = DiagnosticMetric::new("avnu_sell_amount", *amount);
            if let Some(ref symbol) = sell_token_symbol {
                metric = metric.with_label("token", symbol.clone());
            }
            metric = metric.with_label("error_type", Self::category_to_label(category));
            metrics.push(metric);
        }

        metrics
    }

    /// Extracts a string value from metadata.
    fn get_string_value(metadata: &HashMap<String, DiagnosticValue>, key: &str) -> Option<String> {
        match metadata.get(key) {
            Some(DiagnosticValue::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Converts an error category to a label string.
    fn category_to_label(category: &ErrorCategory) -> String {
        match category {
            ErrorCategory::Slippage => "slippage".to_string(),
            ErrorCategory::InsufficientBalance => "insufficient_balance".to_string(),
            ErrorCategory::InvalidInput => "invalid_input".to_string(),
            ErrorCategory::ContractError(name) => format!("contract_error_{}", name),
            ErrorCategory::Allowance => "allowance".to_string(),
            ErrorCategory::RouteNotFound => "route_not_found".to_string(),
            ErrorCategory::Unknown => "unknown".to_string(),
        }
    }
}

#[async_trait]
impl CallMetadataExtractor for AvnuExtractor {
    fn name(&self) -> &'static str {
        "avnu"
    }

    fn can_handle(&self, context: &DiagnosticContext) -> bool {
        // Check if any call targets the AVNU contract
        context.has_call_to(self.contract_address)
    }

    async fn extract(&self, context: &DiagnosticContext) -> CallDiagnostic {
        let category = self.categorize_error(context.error_message);

        // Try to find and extract swap call parameters
        let mut metadata = match self.find_swap_call(context) {
            Some(call) => self.extract_swap_params(call).await,
            None => HashMap::new(),
        };

        // Add user address
        metadata.insert("user_address".to_string(), context.user_address.into());

        // Add contract address for reference
        metadata.insert("contract_address".to_string(), self.contract_address.into());

        // Build extractor-specific metrics
        let metrics = self.build_metrics(&metadata, &category);

        CallDiagnostic {
            contract_name: "avnu",
            error_category: category,
            metadata,
            error_message: context.error_message.to_string(),
            metrics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AVNU Exchange contract address on Starknet mainnet
    const AVNU_ADDRESS: Felt = Felt::from_hex_unchecked("0x04270219d365d6b017231b52e92b3fb5d7c8378b05e9abc97724537a80e93b0f");

    /// ETH token address on Starknet mainnet
    const ETH_TOKEN: Felt = Felt::from_hex_unchecked("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

    /// USDC token address on Starknet mainnet
    const USDC_TOKEN: Felt = Felt::from_hex_unchecked("0x053c91253bc9682c04929ca02ed00b3e423f6710d2ee7e0d5ebb06f3ecf368a8");

    /// Test fixtures for creating swap calls
    mod fixtures {
        use super::*;

        pub fn multi_route_swap_call() -> Call {
            Call {
                to: AVNU_ADDRESS,
                selector: selectors::multi_route_swap(),
                calldata: vec![
                    ETH_TOKEN,              // sell_token_address
                    Felt::from(1000000u64), // sell_token_amount_low
                    Felt::ZERO,             // sell_token_amount_high
                    USDC_TOKEN,             // buy_token_address
                    Felt::from(2000000u64), // buy_token_amount_low
                    Felt::ZERO,             // buy_token_amount_high
                    Felt::from(1900000u64), // buy_token_min_amount_low (slippage)
                    Felt::ZERO,             // buy_token_min_amount_high
                    Felt::from(0x123u64),   // beneficiary
                    Felt::from(30u64),      // integrator_fee_bps
                    Felt::from(0x456u64),   // integrator_fee_recipient
                ],
            }
        }

        pub fn swap_exact_token_to_call() -> Call {
            Call {
                to: AVNU_ADDRESS,
                selector: selectors::swap_exact_token_to(),
                calldata: vec![
                    ETH_TOKEN,              // sell_token_address
                    Felt::from(1000000u64), // sell_token_amount_low
                    Felt::ZERO,             // sell_token_amount_high
                    Felt::from(1200000u64), // sell_token_max_amount_low
                    Felt::ZERO,             // sell_token_max_amount_high
                    USDC_TOKEN,             // buy_token_address
                    Felt::from(2000000u64), // buy_token_amount_low
                    Felt::ZERO,             // buy_token_amount_high
                    Felt::from(0x123u64),   // beneficiary
                    Felt::from(30u64),      // integrator_fee_bps
                    Felt::from(0x456u64),   // integrator_fee_recipient
                ],
            }
        }

        pub fn swap_external_solver_call() -> Call {
            Call {
                to: AVNU_ADDRESS,
                selector: selectors::swap_external_solver(),
                calldata: vec![
                    Felt::from(0x789u64), // user_address
                    ETH_TOKEN,            // sell_token_address
                    USDC_TOKEN,           // buy_token_address
                    Felt::from(0x789u64), // beneficiary
                    Felt::from(0xABCu64), // external_solver_address
                ],
            }
        }

        pub fn call_to_other_contract() -> Call {
            Call {
                to: Felt::from(0x999u64),
                selector: selectors::multi_route_swap(),
                calldata: vec![],
            }
        }
    }

    mod can_handle {
        use super::*;

        #[test]
        fn should_return_true_when_call_targets_avnu_contract() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "error", Felt::from(0x789u64));

            // When
            let result = extractor.can_handle(&context);

            // Then
            assert!(result);
        }

        #[test]
        fn should_return_false_when_call_targets_different_contract() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::call_to_other_contract()];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = extractor.can_handle(&context);

            // Then
            assert!(!result);
        }

        #[test]
        fn should_return_false_when_no_calls_present() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = extractor.can_handle(&context);

            // Then
            assert!(!result);
        }

        #[test]
        fn should_return_true_when_avnu_call_is_among_multiple_calls() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::call_to_other_contract(), fixtures::multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = extractor.can_handle(&context);

            // Then
            assert!(result);
        }
    }

    mod categorize_error {
        use super::*;

        mod slippage_errors {
            use super::*;

            #[test]
            fn should_return_slippage_when_insufficient_tokens_received() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Insufficient tokens received");

                // Then
                assert!(matches!(category, ErrorCategory::Slippage));
            }

            #[test]
            fn should_return_slippage_case_insensitive() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When & Then
                assert!(matches!(extractor.categorize_error("INSUFFICIENT TOKENS RECEIVED"), ErrorCategory::Slippage));
                assert!(matches!(extractor.categorize_error("insufficient tokens received"), ErrorCategory::Slippage));
            }

            #[test]
            fn should_return_slippage_when_invalid_token_max_amount() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Invalid token from max amount");

                // Then
                assert!(matches!(category, ErrorCategory::Slippage));
            }
        }

        mod balance_errors {
            use super::*;

            #[test]
            fn should_return_insufficient_balance_when_balance_too_low() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Token from balance is too low");

                // Then
                assert!(matches!(category, ErrorCategory::InsufficientBalance));
            }

            #[test]
            fn should_return_insufficient_balance_when_amount_is_zero() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Token from amount is 0");

                // Then
                assert!(matches!(category, ErrorCategory::InsufficientBalance));
            }
        }

        mod input_errors {
            use super::*;

            #[test]
            fn should_return_invalid_input_when_routes_empty() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Routes is empty");

                // Then
                assert!(matches!(category, ErrorCategory::InvalidInput));
            }

            #[test]
            fn should_return_invalid_input_when_invalid_token_from() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Invalid token from");

                // Then
                assert!(matches!(category, ErrorCategory::InvalidInput));
            }

            #[test]
            fn should_return_invalid_input_when_invalid_token_to() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Invalid token to");

                // Then
                assert!(matches!(category, ErrorCategory::InvalidInput));
            }
        }

        mod route_errors {
            use super::*;

            #[test]
            fn should_return_route_not_found_when_unknown_exchange() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Unknown exchange");

                // Then
                assert!(matches!(category, ErrorCategory::RouteNotFound));
            }
        }

        mod unknown_errors {
            use super::*;

            #[test]
            fn should_return_contract_error_for_unrecognized_messages() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);

                // When
                let category = extractor.categorize_error("Some random error");

                // Then
                assert!(matches!(category, ErrorCategory::ContractError(ref name) if name == "avnu"));
            }
        }
    }

    mod calculate_slippage_percent {
        use super::*;

        #[test]
        fn should_calculate_correct_percentage() {
            // Given: max = 1000, min = 950 => slippage = 5%
            // When
            let result = AvnuExtractor::calculate_slippage_percent(1000, 950);

            // Then
            assert_eq!(result, Some(5.0));
        }

        #[test]
        fn should_handle_zero_slippage() {
            // Given: max = min = 1000 => slippage = 0%
            // When
            let result = AvnuExtractor::calculate_slippage_percent(1000, 1000);

            // Then
            assert_eq!(result, Some(0.0));
        }

        #[test]
        fn should_return_none_when_max_is_zero() {
            // Given: max = 0 (would cause division by zero)
            // When
            let result = AvnuExtractor::calculate_slippage_percent(0, 0);

            // Then
            assert_eq!(result, None);
        }

        #[test]
        fn should_return_none_when_min_greater_than_max() {
            // Given: min > max (invalid input)
            // When
            let result = AvnuExtractor::calculate_slippage_percent(100, 200);

            // Then
            assert_eq!(result, None);
        }

        #[test]
        fn should_handle_decimal_percentages() {
            // Given: max = 10000, min = 9875 => slippage = 1.25%
            // When
            let result = AvnuExtractor::calculate_slippage_percent(10000, 9875);

            // Then
            assert_eq!(result, Some(1.25));
        }

        #[test]
        fn should_handle_large_values() {
            // Given: large but valid u128 values
            let max: u128 = 1_000_000_000_000_000_000; // 1e18
            let min: u128 = 990_000_000_000_000_000; // 0.99e18, 1% slippage

            // When
            let result = AvnuExtractor::calculate_slippage_percent(max, min);

            // Then
            assert_eq!(result, Some(1.0));
        }
    }

    mod extract {
        use super::*;

        mod multi_route_swap {
            use super::*;

            #[tokio::test]
            async fn should_extract_all_swap_parameters() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "Insufficient tokens received", Felt::from(0x789u64));

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert!(diagnostic.metadata.contains_key("sell_token"));
                assert!(diagnostic.metadata.contains_key("buy_token"));
                assert!(diagnostic.metadata.contains_key("sell_amount_hex"));
                assert!(diagnostic.metadata.contains_key("buy_amount_hex"));
                assert!(diagnostic.metadata.contains_key("buy_min_amount_hex"));
                assert!(diagnostic.metadata.contains_key("beneficiary"));
                assert!(diagnostic.metadata.contains_key("integrator_fee_bps"));
            }

            #[tokio::test]
            async fn should_set_function_name_to_multi_route_swap() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                let function = diagnostic.metadata.get("function");
                assert!(matches!(function, Some(DiagnosticValue::String(s)) if s == "multi_route_swap"));
            }

            #[tokio::test]
            async fn should_categorize_as_slippage_error() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "Insufficient tokens received", Felt::from(0x789u64));

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert_eq!(diagnostic.contract_name, "avnu");
                assert!(matches!(diagnostic.error_category, ErrorCategory::Slippage));
            }

            #[tokio::test]
            async fn should_calculate_slippage_percentage() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                // buy_amount = 2000000, buy_min_amount = 1900000
                // slippage = (2000000 - 1900000) / 2000000 * 100 = 5%
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                let slippage = diagnostic.metadata.get("max_slippage_percent");
                assert!(matches!(slippage, Some(DiagnosticValue::Float(f)) if (*f - 5.0).abs() < 0.01));
            }
        }

        mod swap_exact_token_to {
            use super::*;

            #[tokio::test]
            async fn should_extract_swap_exact_token_to_parameters() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::swap_exact_token_to_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert!(diagnostic.metadata.contains_key("sell_token"));
                assert!(diagnostic.metadata.contains_key("buy_token"));
                assert!(diagnostic.metadata.contains_key("sell_amount_hex"));
                assert!(diagnostic.metadata.contains_key("sell_max_amount_hex"));
                assert!(diagnostic.metadata.contains_key("buy_amount_hex"));
            }

            #[tokio::test]
            async fn should_set_function_name_to_swap_exact_token_to() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::swap_exact_token_to_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                let function = diagnostic.metadata.get("function");
                assert!(matches!(function, Some(DiagnosticValue::String(s)) if s == "swap_exact_token_to"));
            }

            #[tokio::test]
            async fn should_calculate_slippage_percentage() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                // sell_max_amount = 1200000, sell_amount = 1000000
                // slippage = (1200000 - 1000000) / 1200000 * 100 = 16.66%
                let calls = vec![fixtures::swap_exact_token_to_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                let slippage = diagnostic.metadata.get("max_slippage_percent");
                assert!(matches!(slippage, Some(DiagnosticValue::Float(f)) if (*f - 16.66).abs() < 0.01));
            }
        }

        mod swap_external_solver {
            use super::*;

            #[tokio::test]
            async fn should_extract_external_solver_parameters() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::swap_external_solver_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert!(diagnostic.metadata.contains_key("sell_token"));
                assert!(diagnostic.metadata.contains_key("buy_token"));
                assert!(diagnostic.metadata.contains_key("external_solver"));
                assert!(diagnostic.metadata.contains_key("beneficiary"));
            }

            #[tokio::test]
            async fn should_set_function_name_to_swap_external_solver() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::swap_external_solver_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                let function = diagnostic.metadata.get("function");
                assert!(matches!(function, Some(DiagnosticValue::String(s)) if s == "swap_external_solver"));
            }
        }

        mod common_metadata {
            use super::*;

            #[tokio::test]
            async fn should_always_include_user_address() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let user_address = Felt::from(0x789u64);
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "error", user_address);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert!(diagnostic.metadata.contains_key("user_address"));
            }

            #[tokio::test]
            async fn should_always_include_contract_address() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::multi_route_swap_call()];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert!(diagnostic.metadata.contains_key("contract_address"));
            }

            #[tokio::test]
            async fn should_preserve_error_message() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let calls = vec![fixtures::multi_route_swap_call()];
                let error_message = "Custom error message for testing";
                let context = DiagnosticContext::new(&calls, error_message, Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert_eq!(diagnostic.error_message, error_message);
            }
        }

        mod edge_cases {
            use super::*;

            #[tokio::test]
            async fn should_handle_call_with_insufficient_calldata() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let call_with_short_calldata = Call {
                    to: AVNU_ADDRESS,
                    selector: selectors::multi_route_swap(),
                    calldata: vec![Felt::ONE, Felt::TWO], // Only 2 elements instead of 11
                };
                let calls = vec![call_with_short_calldata];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then (should not panic, should have minimal metadata)
                assert_eq!(diagnostic.contract_name, "avnu");
                assert!(diagnostic.metadata.contains_key("function"));
            }

            #[tokio::test]
            async fn should_handle_non_swap_selector() {
                // Given
                let extractor = AvnuExtractor::new(AVNU_ADDRESS);
                let non_swap_call = Call {
                    to: AVNU_ADDRESS,
                    selector: Felt::from(0x12345u64), // Unknown selector
                    calldata: vec![],
                };
                let calls = vec![non_swap_call];
                let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

                // When
                let diagnostic = extractor.extract(&context).await;

                // Then
                assert_eq!(diagnostic.contract_name, "avnu");
                // Should still have user_address and contract_address
                assert!(diagnostic.metadata.contains_key("user_address"));
                assert!(diagnostic.metadata.contains_key("contract_address"));
            }
        }
    }

    mod serialization {
        use super::*;

        #[tokio::test]
        async fn should_serialize_diagnostic_to_json() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "Insufficient tokens received", Felt::from(0x789u64));
            let diagnostic = extractor.extract(&context).await;

            // When
            let json_result = serde_json::to_string(&diagnostic);

            // Then
            assert!(json_result.is_ok());
        }

        #[tokio::test]
        async fn should_include_contract_name_in_json() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "Insufficient tokens received", Felt::ZERO);
            let diagnostic = extractor.extract(&context).await;

            // When
            let json_str = serde_json::to_string(&diagnostic).unwrap();

            // Then
            assert!(json_str.contains("avnu"));
        }

        #[tokio::test]
        async fn should_include_error_category_in_json() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);
            let calls = vec![fixtures::multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "Insufficient tokens received", Felt::ZERO);
            let diagnostic = extractor.extract(&context).await;

            // When
            let json_str = serde_json::to_string(&diagnostic).unwrap();

            // Then
            assert!(json_str.contains("slippage"));
        }
    }

    mod name {
        use super::*;

        #[test]
        fn should_return_avnu() {
            // Given
            let extractor = AvnuExtractor::new(AVNU_ADDRESS);

            // When
            let name = extractor.name();

            // Then
            assert_eq!(name, "avnu");
        }
    }
}
