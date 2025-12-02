//! Core trait and types for call metadata extraction.

use async_trait::async_trait;
use serde::Serialize;
use starknet::core::types::Felt;
use std::collections::HashMap;

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
    /// Contract-specific error with custom code
    ContractError(String),
    /// Approval/allowance issue
    Allowance,
    /// Route or path not found
    RouteNotFound,
    /// Unknown error category
    Unknown,
}

impl ErrorCategory {
    /// Returns a static string representation for metric labels.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::Slippage => "slippage",
            ErrorCategory::InsufficientBalance => "insufficient_balance",
            ErrorCategory::InvalidInput => "invalid_input",
            ErrorCategory::ContractError(_) => "contract_error",
            ErrorCategory::Allowance => "allowance",
            ErrorCategory::RouteNotFound => "route_not_found",
            ErrorCategory::Unknown => "unknown",
        }
    }
}

/// A metric to be emitted by the diagnostic service.
///
/// Extractors can define specific metrics relevant to their domain
/// (e.g., slippage percentage for swap errors, amounts for balance errors).
#[derive(Debug, Clone)]
pub struct DiagnosticMetric {
    /// Metric name (e.g., "avnu_slippage_percent", "avnu_sell_amount")
    pub name: &'static str,
    /// Metric value
    pub value: f64,
    /// Additional labels for this metric (e.g., token symbol, error type)
    pub labels: HashMap<&'static str, String>,
}

impl DiagnosticMetric {
    /// Creates a new diagnostic metric.
    pub fn new(name: &'static str, value: f64) -> Self {
        Self {
            name,
            value,
            labels: HashMap::new(),
        }
    }

    /// Adds a label to the metric.
    pub fn with_label(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.labels.insert(key, value.into());
        self
    }
}

/// The extracted diagnostic information from a failed transaction.
#[derive(Debug, Clone, Serialize)]
pub struct CallDiagnostic {
    /// Name of the contract that produced this diagnostic (e.g., "avnu", "jediswap")
    pub contract_name: &'static str,

    /// Category of the error for filtering/aggregation
    pub error_category: ErrorCategory,

    /// Structured metadata extracted from the calls and error.
    /// Keys should be consistent for each contract (e.g., "sell_token", "buy_token")
    pub metadata: HashMap<String, DiagnosticValue>,

    /// Original error message (for context)
    pub error_message: String,

    /// Specific metrics to emit for this diagnostic.
    /// These are emitted as OpenTelemetry histograms by the DiagnosticService.
    #[serde(skip)]
    pub metrics: Vec<DiagnosticMetric>,
}

/// A typed value for diagnostic metadata to ensure proper serialization.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DiagnosticValue {
    /// Felt value (serialized as hex string)
    Felt(Felt),
    /// String value
    String(String),
    /// Numeric value (integer)
    Number(u128),
    /// Floating point value (for normalized amounts)
    Float(f64),
    /// Boolean value
    Bool(bool),
}

impl From<Felt> for DiagnosticValue {
    fn from(value: Felt) -> Self {
        Self::Felt(value)
    }
}

impl From<String> for DiagnosticValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for DiagnosticValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<u128> for DiagnosticValue {
    fn from(value: u128) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for DiagnosticValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for DiagnosticValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

use crate::diagnostics::DiagnosticContext;

/// Trait for extracting metadata from failed transaction calls.
///
/// Implementors should focus on a specific contract or protocol and extract
/// meaningful information when that contract's calls fail.
///
/// # Implementation Guidelines
///
/// 1. `name()` should return a static string identifying the extractor
/// 2. `can_handle()` should be fast - check contract addresses and error patterns
/// 3. `extract()` is only called if `can_handle()` returns true
/// 4. Use consistent metadata keys for your contract type
///
/// # Example
///
/// ```ignore
/// struct MyDexExtractor {
///     contract_address: Felt,
/// }
///
/// #[async_trait]
/// impl CallMetadataExtractor for MyDexExtractor {
///     fn name(&self) -> &'static str { "mydex" }
///
///     fn can_handle(&self, ctx: &DiagnosticContext) -> bool {
///         ctx.calls.iter().any(|c| c.to == self.contract_address)
///             && ctx.error_message.contains("slippage")
///     }
///
///     async fn extract(&self, ctx: &DiagnosticContext) -> CallDiagnostic {
///         // Extract swap parameters from calldata...
///     }
/// }
/// ```
#[async_trait]
pub trait CallMetadataExtractor: Send + Sync {
    /// Returns the name of this extractor for logging purposes.
    fn name(&self) -> &'static str;

    /// Determines if this extractor can handle the given context.
    ///
    /// This method should be fast as it's called for each extractor in the registry.
    /// Check contract addresses, selectors, and/or error message patterns.
    fn can_handle(&self, context: &DiagnosticContext) -> bool;

    /// Extracts diagnostic information from the context.
    ///
    /// This is only called when `can_handle()` returns `true`.
    /// Extract as much useful information as possible from the calls and error.
    async fn extract(&self, context: &DiagnosticContext) -> CallDiagnostic;
}
