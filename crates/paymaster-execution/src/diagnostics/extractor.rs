//! Core trait and types for call metadata extraction.

use async_trait::async_trait;
use serde::Serialize;
use starknet::core::types::Felt;
use std::collections::HashMap;

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
    fn emit_metrics(&self, _diagnostic: &CallDiagnostic) {}
}
