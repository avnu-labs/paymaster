//! Diagnostic service that manages extractors and orchestrates error analysis.

use super::context::DiagnosticContext;
use super::extractor::{CallDiagnostic, CallMetadataExtractor};
use crate::tokens::TokenService;
use starknet::core::types::Felt;
use std::sync::Arc;
use tracing::{debug, info_span, warn};

/// Service that manages a registry of metadata extractors and analyzes transaction errors.
///
/// The service iterates through registered extractors to find one that can handle
/// the given error context, then extracts and logs diagnostic information.
#[derive(Clone)]
pub struct DiagnosticService {
    extractors: Vec<Arc<dyn CallMetadataExtractor>>,
}

impl DiagnosticService {
    /// Creates a diagnostic service pre-configured with all known extractors.
    ///
    /// Currently includes:
    /// - AVNU Exchange extractor (mainnet address)
    pub fn new(chain_id: Felt) -> Self {
        let token_service = TokenService::for_chain(chain_id);
        use super::extractors::{AvnuExtractor, AVNU_EXCHANGE_ADDRESS};

        Self::empty().with_extractor(AvnuExtractor::with_token_service(AVNU_EXCHANGE_ADDRESS, token_service))
    }

    /// Creates an empty diagnostic service with no extractors.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self { extractors: Vec::new() }
    }

    #[cfg(not(test))]
    fn empty() -> Self {
        Self { extractors: Vec::new() }
    }

    /// Adds an extractor to the registry.
    ///
    /// Extractors are checked in the order they are added.
    #[cfg(test)]
    pub(crate) fn with_extractor<E: CallMetadataExtractor + 'static>(mut self, extractor: E) -> Self {
        self.extractors.push(Arc::new(extractor));
        self
    }

    #[cfg(not(test))]
    fn with_extractor<E: CallMetadataExtractor + 'static>(mut self, extractor: E) -> Self {
        self.extractors.push(Arc::new(extractor));
        self
    }

    /// Analyzes the given context and returns a diagnostic if any extractor matches.
    ///
    /// Returns `None` if no extractor can handle the context.
    pub async fn analyze(&self, context: &DiagnosticContext<'_>) -> Option<CallDiagnostic> {
        for extractor in &self.extractors {
            if extractor.can_handle(context) {
                return Some(extractor.extract(context).await);
            }
        }
        None
    }

    /// Analyzes and logs the diagnostic with structured fields.
    ///
    /// This is the primary method to use when handling transaction errors.
    /// It extracts diagnostics and logs them with appropriate tracing spans
    /// for CloudWatch/OTEL ingestion.
    pub async fn analyze_and_log(&self, context: &DiagnosticContext<'_>) {
        if let Some(diagnostic) = self.analyze(context).await {
            self.log_diagnostic(&diagnostic);
        }
    }

    /// Logs a diagnostic with structured tracing fields and emits OpenTelemetry metrics.
    fn log_diagnostic(&self, diagnostic: &CallDiagnostic) {
        let span = info_span!(
            "transaction_diagnostic",
            contract = diagnostic.contract_name,
            category = ?diagnostic.error_category,
        );

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
    ///
    /// Emits:
    /// 1. A counter `diagnostic_error` with category and contract labels
    /// 2. Extractor-specific histogram metrics from `diagnostic.metrics`
    fn emit_metrics(&self, diagnostic: &CallDiagnostic) {
        let category = diagnostic.error_category.as_str();

        // Emit main counter with category and contract labels
        debug!(monotonic_counter.diagnostic_error = 1, category = category, contract = diagnostic.contract_name);

        // Emit extractor-specific histogram metrics
        for m in &diagnostic.metrics {
            debug!(histogram.diagnostic_metric = m.value, name = m.name, labels = ?m.labels);
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
    use crate::diagnostics::{DiagnosticValue, ErrorCategory};
    use async_trait::async_trait;
    use starknet::core::types::{Call, Felt};
    use std::collections::HashMap;

    /// Test helper: Extractor that can be configured to handle or not handle contexts
    struct ConfigurableExtractor {
        name: &'static str,
        should_handle: bool,
    }

    impl ConfigurableExtractor {
        fn handling(name: &'static str) -> Self {
            Self { name, should_handle: true }
        }

        fn not_handling(name: &'static str) -> Self {
            Self { name, should_handle: false }
        }
    }

    #[async_trait]
    impl CallMetadataExtractor for ConfigurableExtractor {
        fn name(&self) -> &'static str {
            self.name
        }

        fn can_handle(&self, _context: &DiagnosticContext) -> bool {
            self.should_handle
        }

        async fn extract(&self, context: &DiagnosticContext) -> CallDiagnostic {
            let mut metadata = HashMap::new();
            metadata.insert("extractor_name".to_string(), DiagnosticValue::String(self.name.to_string()));

            CallDiagnostic {
                contract_name: self.name,
                error_category: ErrorCategory::Unknown,
                metadata,
                error_message: context.error_message.to_string(),
                metrics: Vec::new(),
            }
        }
    }

    mod new {
        use super::*;

        // Mainnet chain ID for tests
        const TEST_CHAIN_ID: Felt = Felt::from_hex_unchecked("0x534e5f4d41494e");

        #[test]
        fn should_create_service_with_avnu_extractor() {
            // Given & When
            let service = DiagnosticService::new(TEST_CHAIN_ID);

            // Then - AVNU extractor is automatically added
            assert_eq!(service.extractor_count(), 1);
        }
    }

    mod empty {
        use super::*;

        #[test]
        fn should_create_service_with_no_extractors() {
            // Given & When
            let service = DiagnosticService::empty();

            // Then
            assert_eq!(service.extractor_count(), 0);
        }
    }

    mod with_extractor {
        use super::*;

        #[test]
        fn should_add_extractor_to_registry() {
            // Given
            let service = DiagnosticService::empty();

            // When
            let service = service.with_extractor(ConfigurableExtractor::handling("test"));

            // Then
            assert_eq!(service.extractor_count(), 1);
        }

        #[test]
        fn should_allow_chaining_multiple_extractors() {
            // Given
            let service = DiagnosticService::empty();

            // When
            let service = service
                .with_extractor(ConfigurableExtractor::handling("first"))
                .with_extractor(ConfigurableExtractor::handling("second"))
                .with_extractor(ConfigurableExtractor::handling("third"));

            // Then
            assert_eq!(service.extractor_count(), 3);
        }
    }

    mod analyze {
        use super::*;

        #[tokio::test]
        async fn should_return_none_when_no_extractors_registered() {
            // Given
            let service = DiagnosticService::empty();
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = service.analyze(&context).await;

            // Then
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn should_return_none_when_no_extractor_can_handle() {
            // Given
            let service = DiagnosticService::empty()
                .with_extractor(ConfigurableExtractor::not_handling("first"))
                .with_extractor(ConfigurableExtractor::not_handling("second"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = service.analyze(&context).await;

            // Then
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn should_return_diagnostic_when_extractor_matches() {
            // Given
            let service = DiagnosticService::empty().with_extractor(ConfigurableExtractor::handling("test"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "test error message", Felt::ZERO);

            // When
            let result = service.analyze(&context).await;

            // Then
            assert!(result.is_some());
            let diagnostic = result.unwrap();
            assert_eq!(diagnostic.contract_name, "test");
            assert_eq!(diagnostic.error_message, "test error message");
        }

        #[tokio::test]
        async fn should_return_first_matching_extractor_result_when_multiple_match() {
            // Given
            let service = DiagnosticService::empty()
                .with_extractor(ConfigurableExtractor::handling("first"))
                .with_extractor(ConfigurableExtractor::handling("second"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = service.analyze(&context).await;

            // Then
            assert!(result.is_some());
            assert_eq!(result.unwrap().contract_name, "first");
        }

        #[tokio::test]
        async fn should_skip_non_matching_extractors_and_use_first_match() {
            // Given
            let service = DiagnosticService::empty()
                .with_extractor(ConfigurableExtractor::not_handling("first"))
                .with_extractor(ConfigurableExtractor::not_handling("second"))
                .with_extractor(ConfigurableExtractor::handling("third"))
                .with_extractor(ConfigurableExtractor::handling("fourth"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = service.analyze(&context).await;

            // Then
            assert!(result.is_some());
            assert_eq!(result.unwrap().contract_name, "third");
        }
    }

    mod extractor_count {
        use super::*;

        #[test]
        fn should_return_zero_for_empty_service() {
            // Given
            let service = DiagnosticService::empty();

            // When
            let count = service.extractor_count();

            // Then
            assert_eq!(count, 0);
        }

        #[test]
        fn should_return_correct_count_after_adding_extractors() {
            // Given
            let service = DiagnosticService::empty()
                .with_extractor(ConfigurableExtractor::handling("one"))
                .with_extractor(ConfigurableExtractor::handling("two"));

            // When
            let count = service.extractor_count();

            // Then
            assert_eq!(count, 2);
        }
    }

    mod analyze_and_log {
        use super::*;

        #[tokio::test]
        async fn should_not_panic_when_no_extractors_match() {
            // Given
            let service = DiagnosticService::empty().with_extractor(ConfigurableExtractor::not_handling("test"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When & Then (should not panic)
            service.analyze_and_log(&context).await;
        }

        #[tokio::test]
        async fn should_not_panic_when_extractor_matches() {
            // Given
            let service = DiagnosticService::empty().with_extractor(ConfigurableExtractor::handling("test"));

            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When & Then (should not panic)
            service.analyze_and_log(&context).await;
        }
    }
}
