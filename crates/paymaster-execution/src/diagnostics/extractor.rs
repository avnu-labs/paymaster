//! Core trait and types for call metadata extraction.

use async_trait::async_trait;
use serde::Serialize;
use starknet::core::types::Felt;
use std::collections::HashMap;

/// A metric to be emitted by the diagnostic service.
///
/// Extractors can define specific metrics relevant to their domain
/// (e.g., slippage percentage for swap errors, amounts for balance errors).
#[derive(Debug, Clone)]
pub struct DiagnosticMetric {
    /// Metric name (e.g., "avnu_slippage_percent", "avnu_sell_amount")
    pub name: String,
    /// Metric value
    pub value: f64,
    /// Additional labels for this metric (e.g., token symbol, error type)
    pub labels: HashMap<String, String>,
}

impl DiagnosticMetric {
    /// Creates a new diagnostic metric.
    pub fn new(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            value,
            labels: HashMap::new(),
        }
    }

    /// Adds a label to the metric.
    pub fn with_label(mut self, key: &str, value: impl Into<String>) -> Self {
        self.labels.insert(key.to_string(), value.into());
        self
    }
}

/// The extracted diagnostic information from a failed transaction.
#[derive(Debug, Clone, Serialize)]
pub struct CallDiagnostic {
    /// Name of the contract that produced this diagnostic (e.g., "avnu", "jediswap")
    pub contract_name: String,

    /// Category of the error for filtering/aggregation
    pub error_category: String,

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

#[async_trait]
pub trait CallMetadataExtractor: Send + Sync {
    fn name(&self) -> String;
    async fn try_extract(&self, context: &DiagnosticContext) -> Option<CallDiagnostic>;
}
