//! Diagnostic service that manages extractors and orchestrates error analysis.

use super::context::DiagnosticContext;
use super::extractor::{CallDiagnostic, CallMetadataExtractor};
use super::extractors::{AvnuExtractor, AVNU_EXCHANGE_ADDRESS_MAINNET, AVNU_EXCHANGE_ADDRESS_SEPOLIA};
use crate::tokens::TokenClient;
use opentelemetry::{global, KeyValue};
use paymaster_common::metric;
use paymaster_starknet::transaction::Calls;
use paymaster_starknet::ChainID;
use starknet::core::types::Felt;
use std::sync::Arc;
use tracing::{info_span, warn};

/// Service that manages a registry of metadata extractors and analyzes transaction errors.
///
/// The service iterates through registered extractors to find one that can handle
/// the given error context, then extracts and logs diagnostic information.
#[derive(Clone)]
pub struct DiagnosticClient {
    extractors: Vec<Arc<dyn CallMetadataExtractor>>,
}
impl DiagnosticClient {
    /// Creates a diagnostic service pre-configured with all known extractors.
    ///
    /// Currently includes:
    /// - AVNU Exchange extractor (mainnet address)
    pub fn new(chain_id: ChainID) -> Self {
        let token_client = TokenClient::new(chain_id);
        let avnu_contract_address = match chain_id {
            ChainID::Sepolia => AVNU_EXCHANGE_ADDRESS_SEPOLIA,
            ChainID::Mainnet => AVNU_EXCHANGE_ADDRESS_MAINNET,
        };
        Self {
            extractors: vec![Arc::new(AvnuExtractor::new(avnu_contract_address, token_client))],
        }
    }

    /// Analyzes and logs the diagnostic with structured fields.
    ///
    /// This is the primary method to use when handling transaction errors.
    /// It extracts diagnostics and logs them with appropriate tracing spans
    /// for CloudWatch/OTEL ingestion.
    pub async fn report(&self, calls: &Calls, user_address: Felt, error_message: String) {
        let context = DiagnosticContext::new(&calls, &error_message, user_address);
        for diagnostic in self.analyze(&context).await {
            self.log_diagnostic(&diagnostic);
        }
    }

    async fn analyze(&self, context: &DiagnosticContext) -> Vec<CallDiagnostic> {
        let mut diagnostics = Vec::new();
        for extractor in &self.extractors {
            if let Some(diagnostic) = extractor.try_extract(context).await {
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }

    fn log_diagnostic(&self, diagnostic: &CallDiagnostic) {
        let span = info_span!("transaction_diagnostic", contract = diagnostic.contract_name, category = diagnostic.error_category);

        let _guard = span.enter();

        // Emit OpenTelemetry metrics
        self.emit_metrics(diagnostic);

        // Log the full diagnostic as JSON for CloudWatch analysis
        match serde_json::to_string(&diagnostic) {
            Ok(json) => {
                warn!(
                    diagnostic = %json,
                    "Transaction simulation failed with extracted context"
                );
            },
            Err(_) => {
                warn!(
                    error = %diagnostic.error_message,
                    "Transaction simulation failed (diagnostic serialization error)"
                );
            },
        }
    }

    /// Emits OpenTelemetry metrics for a diagnostic.
    fn emit_metrics(&self, diagnostic: &CallDiagnostic) {
        // Emit main counter with category and contract labels
        metric!(
            counter[diagnostic_error] = 1,
            category = diagnostic.error_category.as_str(),
            contract = diagnostic.contract_name
        );

        // Emit extractor-specific histogram metrics
        let meter = global::meter("tracing");
        for m in &diagnostic.metrics {
            let histogram = meter.f64_histogram(m.name.clone()).build();
            let attributes: Vec<KeyValue> = m.labels.iter().map(|(k, v)| KeyValue::new(k.clone(), v.clone())).collect();
            histogram.record(m.value, &attributes);
        }
    }

    /// Returns the number of registered extractors.
    pub fn extractor_count(&self) -> usize {
        self.extractors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::extractors::AVNU_EXCHANGE_ADDRESS_MAINNET;
    use starknet::core::types::{Call, Felt};

    fn avnu_call() -> Call {
        Call {
            to: AVNU_EXCHANGE_ADDRESS_MAINNET,
            selector: Felt::from(0x1234u64),
            calldata: vec![],
        }
    }

    fn other_contract_call() -> Call {
        Call {
            to: Felt::from(0x999u64),
            selector: Felt::from(0x1234u64),
            calldata: vec![],
        }
    }

    mod new {
        use super::*;

        #[test]
        fn should_register_avnu_extractor() {
            let client = DiagnosticClient::new(ChainID::Mainnet);
            assert_eq!(client.extractor_count(), 1);
        }
    }

    mod analyze {
        use super::*;

        #[tokio::test]
        async fn should_extract_diagnostic_for_avnu_call() {
            let client = DiagnosticClient::new(ChainID::Mainnet);
            let context = DiagnosticContext::new(&[avnu_call()], "Insufficient tokens received", Felt::from(0x123u64));

            let diagnostics = client.analyze(&context).await;

            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].contract_name, "avnu");
            assert_eq!(diagnostics[0].error_category, "slippage");
        }

        #[tokio::test]
        async fn should_return_empty_for_unknown_contract() {
            let client = DiagnosticClient::new(ChainID::Mainnet);
            let context = DiagnosticContext::new(&[other_contract_call()], "Some error", Felt::from(0x123u64));

            let diagnostics = client.analyze(&context).await;

            assert!(diagnostics.is_empty());
        }
    }
}
