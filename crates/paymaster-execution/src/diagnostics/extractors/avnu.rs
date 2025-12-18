//! AVNU Exchange contract metadata extractor.
//!
//! Extracts diagnostic information from failed AVNU swap transactions,
//! particularly focusing on slippage errors and swap parameters.

use crate::diagnostics::extractors::Metadata;
use crate::diagnostics::{CallDiagnostic, CallMetadataExtractor, DiagnosticContext, DiagnosticMetric, DiagnosticValue};
use crate::tokens::TokenClient;
use crate::Error;
use async_trait::async_trait;
use paymaster_starknet::math::felt_to_u128;
use serde::Serialize;
use starknet::core::types::{Call, Felt};
use starknet::macros::selector;
use tracing::warn;

/// AVNU Exchange contract address on Starknet mainnet.
pub const AVNU_EXCHANGE_ADDRESS_MAINNET: Felt = Felt::from_hex_unchecked("0x04270219d365d6b017231b52e92b3fb5d7c8378b05e9abc97724537a80e93b0f");
pub const AVNU_EXCHANGE_ADDRESS_SEPOLIA: Felt = Felt::from_hex_unchecked("0x02c56e8b00dbe2a71e57472685378fc8988bba947e9a99b26a00fade2b4fe7c2");

/// Slippage exceeded - buy_token_min_amount > buy_token_final_amount
const INSUFFICIENT_TOKENS_RECEIVED: &str = "insufficient tokens received";

/// Slippage exceeded - sell_token_max_amount < sell_token_amount in swap_exact_token_to
const INVALID_TOKEN_MAX_AMOUNT: &str = "invalid token from max amount";

/// User doesn't have enough tokens to sell
const TOKEN_BALANCE_TOO_LOW: &str = "token from balance is too low";

/// Token amount is zero
const TOKEN_AMOUNT_ZERO: &str = "token from amount is 0";

/// Routes array is empty
const ROUTES_EMPTY: &str = "routes is empty";

/// First route sell token doesn't match
const INVALID_TOKEN_FROM: &str = "invalid token from";

/// Last route buy token doesn't match
const INVALID_TOKEN_TO: &str = "invalid token to";

/// Unknown exchange in routes
const UNKNOWN_EXCHANGE: &str = "unknown exchange";

const MULTI_ROUTE_SWAP_SELECTOR: Felt = selector!("multi_route_swap");
const SWAP_EXACT_TOKEN_TO_SELECTOR: Felt = selector!("swap_exact_token_to");
const SWAP_EXTERNAL_SOLVER_SELECTOR: Felt = selector!("swap_external_solver");

/// Categorization of error types for metrics and filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Slippage exceeded in a swap operation
    Slippage,
    /// Insufficient balance for the operation
    InsufficientBalance,
    /// Invalid input parameters
    InvalidInput,
    /// Route or path not found
    RouteNotFound,
    /// Unknown error category
    Unknown,
}

impl ErrorCategory {
    pub fn new(error: &str) -> Self {
        if ErrorCategory::is_slippage_error(error) {
            ErrorCategory::Slippage
        } else if ErrorCategory::is_balance_error(error) {
            ErrorCategory::InsufficientBalance
        } else if ErrorCategory::is_input_error(error) {
            ErrorCategory::InvalidInput
        } else if ErrorCategory::is_route_not_found_error(error) {
            ErrorCategory::RouteNotFound
        } else {
            ErrorCategory::Unknown
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::Slippage => "slippage",
            ErrorCategory::InsufficientBalance => "insufficient_balance",
            ErrorCategory::InvalidInput => "invalid_input",
            ErrorCategory::RouteNotFound => "route_not_found",
            ErrorCategory::Unknown => "unknown",
        }
    }

    fn is_slippage_error(error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(INSUFFICIENT_TOKENS_RECEIVED) || lower.contains(INVALID_TOKEN_MAX_AMOUNT)
    }

    fn is_balance_error(error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(TOKEN_BALANCE_TOO_LOW) || lower.contains(TOKEN_AMOUNT_ZERO)
    }

    fn is_input_error(error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(ROUTES_EMPTY) || lower.contains(INVALID_TOKEN_FROM) || lower.contains(INVALID_TOKEN_TO)
    }

    fn is_route_not_found_error(error: &str) -> bool {
        let lower = error.to_lowercase();
        lower.contains(UNKNOWN_EXCHANGE)
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
    contract_address: Felt,
    token_client: TokenClient,
}

impl AvnuExtractor {
    /// Creates a new AVNU extractor for the given contract address.
    pub fn new(contract_address: Felt, token_client: TokenClient) -> Self {
        Self { contract_address, token_client }
    }

    /// Computes slippage percentage from u256 low parts (max_amount, min_amount).
    ///
    /// Formula: ((max - min) / max) * 100
    ///
    /// Only uses the low part of u256 values since actual amounts never exceed u128.
    /// Returns None if calculation would divide by zero.
    fn try_compute_slippage_percent(max_low: u128, min_low: u128) -> Option<f64> {
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
    async fn insert_token_info(&self, prefix: &str, metadata: &mut Metadata, token_address: Felt) {
        if let Some(token_info) = self.token_client.get_token(token_address).await {
            metadata.insert(format!("{prefix}_symbol"), token_info.symbol.clone());
            metadata.insert(format!("{prefix}_name"), token_info.name.clone());
        }
    }

    /// Adds an amount to metadata, both normalized and raw hex.
    ///
    /// When token info is available, also adds:
    /// - `{key}`: Normalized amount as Float (e.g., 1.5 instead of 1500000000000000000)
    async fn insert_token_amount(&self, key: &str, metadata: &mut Metadata, token_address: Felt, raw_amount: Felt) {
        if let Some(amount) = self.token_client.try_normalize_amount(token_address, raw_amount).await {
            metadata.insert(key.to_string(), amount);
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
    async fn extract_multi_route_swap_params(&self, call: &Call) -> Result<Metadata, Error> {
        let mut metadata = Metadata::new();
        let calldata = &call.calldata;

        if calldata.len() < 11 {
            return Ok(metadata);
        }

        let sell_token = calldata[0];
        let sell_amount_raw = calldata[1];
        let buy_token = calldata[3];
        let buy_amount_raw = calldata[4];
        let buy_min_amount_raw = calldata[6];

        metadata.insert("sell_token", sell_token);
        metadata.insert("buy_token", buy_token);
        metadata.insert("sell_amount_hex", format!("0x{:x}", sell_amount_raw));
        metadata.insert("buy_amount_hex", format!("0x{:x}", buy_amount_raw));
        metadata.insert("buy_min_amount_hex", format!("0x{:x}", buy_min_amount_raw));
        metadata.insert("beneficiary", calldata[8]);
        metadata.insert("integrator_fee_bps", felt_to_u128(calldata[9])?);
        metadata.insert("integrator_fee_recipient", calldata[10]);

        self.insert_token_info("sell_token", &mut metadata, sell_token).await;
        self.insert_token_info("buy_token", &mut metadata, buy_token).await;
        self.insert_token_amount("sell_amount", &mut metadata, sell_token, sell_amount_raw)
            .await;
        self.insert_token_amount("buy_amount", &mut metadata, buy_token, buy_amount_raw)
            .await;
        self.insert_token_amount("buy_min_amount", &mut metadata, buy_token, buy_min_amount_raw)
            .await;

        // Calculate slippage percentage: ((buy_amount - buy_min_amount) / buy_amount) * 100
        if let Some(slippage_pct) = Self::try_compute_slippage_percent(felt_to_u128(buy_amount_raw)?, felt_to_u128(buy_min_amount_raw)?) {
            metadata.insert("max_slippage_percent", slippage_pct);
        }

        Ok(metadata)
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
    async fn extract_swap_exact_token_to_params(&self, call: &Call) -> Result<Metadata, Error> {
        let mut metadata = Metadata::new();
        let calldata = &call.calldata;

        if calldata.len() < 11 {
            return Ok(metadata);
        }

        let sell_token = calldata[0];
        let sell_amount_raw = calldata[1];
        let sell_max_amount_raw = calldata[3];
        let buy_token = calldata[5];
        let buy_amount_raw = calldata[6];

        metadata.insert("sell_token", sell_token);
        metadata.insert("buy_token", buy_token);
        metadata.insert("sell_amount_hex", format!("0x{:x}", sell_amount_raw));
        metadata.insert("sell_max_amount_hex", format!("0x{:x}", sell_max_amount_raw));
        metadata.insert("buy_amount_hex", format!("0x{:x}", buy_amount_raw));
        metadata.insert("beneficiary", calldata[8]);
        metadata.insert("integrator_fee_bps", felt_to_u128(calldata[9])?);
        metadata.insert("integrator_fee_recipient", calldata[10]);

        self.insert_token_info("sell_token", &mut metadata, sell_token).await;
        self.insert_token_info("buy_token", &mut metadata, buy_token).await;
        self.insert_token_amount("sell_amount", &mut metadata, sell_token, sell_amount_raw)
            .await;
        self.insert_token_amount("sell_max_amount", &mut metadata, sell_token, sell_max_amount_raw)
            .await;
        self.insert_token_amount("buy_amount", &mut metadata, buy_token, buy_amount_raw)
            .await;

        // Calculate slippage percentage: ((sell_max_amount - sell_amount) / sell_max_amount) * 100
        if let Some(slippage_pct) = Self::try_compute_slippage_percent(felt_to_u128(sell_max_amount_raw)?, felt_to_u128(sell_amount_raw)?) {
            metadata.insert("max_slippage_percent", slippage_pct);
        }

        Ok(metadata)
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
    fn extract_swap_external_solver_params(&self, call: &Call) -> Metadata {
        let mut metadata = Metadata::new();
        let calldata = &call.calldata;

        if calldata.len() < 5 {
            return metadata;
        }
        metadata.insert("user_address", calldata[0]);
        metadata.insert("sell_token", calldata[1]);
        metadata.insert("buy_token", calldata[2]);
        metadata.insert("beneficiary", calldata[3]);
        metadata.insert("external_solver", calldata[4]);
        metadata
    }

    /// Finds the first AVNU swap call in the context.
    fn find_swap_call<'a>(&self, context: &'a DiagnosticContext) -> Option<&'a Call> {
        context
            .calls_to(self.contract_address)
            .find(|call| call.selector == MULTI_ROUTE_SWAP_SELECTOR || call.selector == SWAP_EXACT_TOKEN_TO_SELECTOR || call.selector == SWAP_EXTERNAL_SOLVER_SELECTOR)
    }

    /// Extracts swap parameters based on the function selector.
    async fn extract_swap_params(&self, call: &Call) -> Result<Metadata, Error> {
        let mut metadata = if call.selector == MULTI_ROUTE_SWAP_SELECTOR {
            self.extract_multi_route_swap_params(call).await?
        } else if call.selector == SWAP_EXACT_TOKEN_TO_SELECTOR {
            self.extract_swap_exact_token_to_params(call).await?
        } else if call.selector == SWAP_EXTERNAL_SOLVER_SELECTOR {
            self.extract_swap_external_solver_params(call)
        } else {
            Metadata::new()
        };

        // Add the function name for clarity
        let function_name = if call.selector == MULTI_ROUTE_SWAP_SELECTOR {
            "multi_route_swap"
        } else if call.selector == SWAP_EXACT_TOKEN_TO_SELECTOR {
            "swap_exact_token_to"
        } else if call.selector == SWAP_EXTERNAL_SOLVER_SELECTOR {
            "swap_external_solver"
        } else {
            "unknown"
        };

        metadata.insert("function", function_name);
        Ok(metadata)
    }

    /// Builds AVNU-specific metrics from extracted metadata.
    ///
    /// Emits:
    /// - `avnu_slippage_percent`: Max slippage percentage for slippage errors
    /// - `avnu_sell_amount`: Normalized sell amount for swap errors
    /// - `avnu_buy_amount`: Normalized buy amount for swap errors
    fn build_metrics(&self, metadata: &Metadata, category: &ErrorCategory) -> Vec<DiagnosticMetric> {
        let mut metrics = Vec::new();

        // Get token symbols for labels (if available)
        let sell_token_symbol = metadata.get_string_value("sell_token_symbol");
        let buy_token_symbol = metadata.get_string_value("buy_token_symbol");

        // Slippage metric - useful for understanding slippage distribution in errors
        if let Some(DiagnosticValue::Float(slippage)) = metadata.0.get("max_slippage_percent") {
            let mut metric = DiagnosticMetric::new("avnu_slippage_percent", *slippage);
            if let Some(symbol) = sell_token_symbol.clone() {
                metric = metric.with_label("sell_token", symbol.clone());
            }
            if let Some(symbol) = buy_token_symbol {
                metric = metric.with_label("buy_token", symbol.clone());
            }
            metrics.push(metric);
        }

        // Sell amount metric - useful for understanding which amounts fail
        if let Some(DiagnosticValue::Float(amount)) = metadata.0.get("sell_amount") {
            let mut metric = DiagnosticMetric::new("avnu_sell_amount", *amount);
            if let Some(symbol) = sell_token_symbol {
                metric = metric.with_label("token", symbol.clone());
            }
            metric = metric.with_label("error_type", category.as_str());
            metrics.push(metric);
        }

        metrics
    }
}

#[async_trait]
impl CallMetadataExtractor for AvnuExtractor {
    fn name(&self) -> String {
        "avnu".to_string()
    }

    async fn try_extract(&self, context: &DiagnosticContext) -> Option<CallDiagnostic> {
        if !context.has_call_to(self.contract_address) {
            return None;
        }

        let category = ErrorCategory::new(&context.error_message);

        // Try to find and extract swap call parameters
        let mut metadata = match self.find_swap_call(context) {
            Some(call) => match self.extract_swap_params(call).await {
                Ok(m) => m,
                Err(error) => {
                    warn!("Failed to extract avnu swap params. {:?}", error);
                    return None;
                },
            },
            None => Metadata::new(),
        };

        // Add user address
        metadata.insert("user_address", context.user_address);

        // Add contract address for reference
        metadata.insert("contract_address", self.contract_address);

        // Build extractor-specific metrics
        let metrics = self.build_metrics(&metadata, &category);

        Some(CallDiagnostic {
            contract_name: "avnu".to_string(),
            error_category: category.as_str().to_string(),
            metadata: metadata.0,
            error_message: context.error_message.to_string(),
            metrics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paymaster_starknet::constants::Token;
    use paymaster_starknet::ChainID;

    fn extractor() -> AvnuExtractor {
        AvnuExtractor::new(AVNU_EXCHANGE_ADDRESS_MAINNET, TokenClient::mainnet())
    }

    fn multi_route_swap_call() -> Call {
        Call {
            to: AVNU_EXCHANGE_ADDRESS_MAINNET,
            selector: MULTI_ROUTE_SWAP_SELECTOR,
            calldata: vec![
                Token::ETH_ADDRESS,
                Felt::from(1000000u64),
                Felt::ZERO,
                Token::usdc(&ChainID::Mainnet).address,
                Felt::from(2000000u64),
                Felt::ZERO,
                Felt::from(1900000u64),
                Felt::ZERO,
                Felt::from(0x123u64),
                Felt::from(30u64),
                Felt::from(0x456u64),
            ],
        }
    }

    fn swap_exact_token_to_call() -> Call {
        Call {
            to: AVNU_EXCHANGE_ADDRESS_MAINNET,
            selector: SWAP_EXACT_TOKEN_TO_SELECTOR,
            calldata: vec![
                Token::ETH_ADDRESS,
                Felt::from(1000000u64),
                Felt::ZERO,
                Felt::from(1200000u64),
                Felt::ZERO,
                Token::usdc(&ChainID::Mainnet).address,
                Felt::from(2000000u64),
                Felt::ZERO,
                Felt::from(0x123u64),
                Felt::from(30u64),
                Felt::from(0x456u64),
            ],
        }
    }

    mod calculate_slippage_percent {
        use super::*;

        #[test]
        fn should_return_correct_percentage_when_valid_inputs() {
            assert_eq!(AvnuExtractor::try_compute_slippage_percent(1000, 950), Some(5.0));
            assert_eq!(AvnuExtractor::try_compute_slippage_percent(1000, 1000), Some(0.0));
            assert_eq!(
                AvnuExtractor::try_compute_slippage_percent(1_000_000_000_000_000_000, 990_000_000_000_000_000),
                Some(1.0)
            );
        }

        #[test]
        fn should_return_none_when_max_is_zero() {
            assert_eq!(AvnuExtractor::try_compute_slippage_percent(0, 0), None);
        }

        #[test]
        fn should_return_none_when_min_greater_than_max() {
            assert_eq!(AvnuExtractor::try_compute_slippage_percent(100, 200), None);
        }
    }

    mod try_extract {
        use super::*;

        #[tokio::test]
        async fn should_return_none_when_no_avnu_call() {
            let extractor = extractor();
            let other_call = Call {
                to: Felt::from(0x999u64),
                selector: MULTI_ROUTE_SWAP_SELECTOR,
                calldata: vec![],
            };
            let context = DiagnosticContext::new(&vec![other_call], "error", Felt::ZERO);

            assert!(extractor.try_extract(&context).await.is_none());
        }

        #[tokio::test]
        async fn should_extract_params_when_multi_route_swap() {
            let extractor = extractor();
            let calls = vec![multi_route_swap_call()];
            let context = DiagnosticContext::new(&calls, "insufficient tokens received", Felt::from(0x789u64));

            let diagnostic = extractor.try_extract(&context).await.unwrap();

            assert_eq!(diagnostic.contract_name, "avnu");
            assert_eq!(diagnostic.error_category, "slippage".to_string());
            assert!(matches!(diagnostic.metadata.get("function"), Some(DiagnosticValue::String(s)) if s == "multi_route_swap"));
            assert!(matches!(diagnostic.metadata.get("sell_token"), Some(DiagnosticValue::Felt(addr)) if *addr == Token::ETH_ADDRESS));
            assert!(matches!(diagnostic.metadata.get("buy_token"), Some(DiagnosticValue::Felt(addr)) if *addr == Token::usdc(&ChainID::Mainnet).address));

            // Slippage: (2000000 - 1900000) / 2000000 * 100 = 5%
            let slippage = diagnostic.metadata.get("max_slippage_percent");
            assert!(matches!(slippage, Some(DiagnosticValue::Float(f)) if (*f - 5.0).abs() < 0.01));
        }

        #[tokio::test]
        async fn should_extract_params_when_swap_exact_token_to() {
            let extractor = extractor();
            let calls = vec![swap_exact_token_to_call()];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            let diagnostic = extractor.try_extract(&context).await.unwrap();

            assert!(matches!(diagnostic.metadata.get("function"), Some(DiagnosticValue::String(s)) if s == "swap_exact_token_to"));
            assert!(diagnostic.metadata.contains_key("sell_max_amount_hex"));

            // Slippage: (1200000 - 1000000) / 1200000 * 100 = 16.66%
            let slippage = diagnostic.metadata.get("max_slippage_percent");
            assert!(matches!(slippage, Some(DiagnosticValue::Float(f)) if (*f - 16.66).abs() < 0.01));
        }

        #[tokio::test]
        async fn should_return_basic_metadata_when_calldata_insufficient() {
            let extractor = extractor();
            let short_call = Call {
                to: AVNU_EXCHANGE_ADDRESS_MAINNET,
                selector: MULTI_ROUTE_SWAP_SELECTOR,
                calldata: vec![Felt::ONE, Felt::TWO],
            };
            let context = DiagnosticContext::new(&vec![short_call], "error", Felt::ZERO);

            let diagnostic = extractor.try_extract(&context).await.unwrap();

            assert_eq!(diagnostic.contract_name, "avnu");
            assert!(diagnostic.metadata.contains_key("user_address"));
            assert!(diagnostic.metadata.contains_key("contract_address"));
        }
    }
}
