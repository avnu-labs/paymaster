//! Diagnostic context containing all information needed for error analysis.

use starknet::core::types::{Call, Felt};

/// Context provided to extractors for analyzing a failed transaction.
///
/// Contains the original calls that were attempted, the error that occurred,
/// and relevant transaction metadata.
#[derive(Debug)]
pub struct DiagnosticContext {
    /// The calls that were part of the failed transaction
    pub calls: Vec<Call>,

    /// The error message from the simulation/execution failure
    pub error_message: String,

    /// The user's account address
    pub user_address: Felt,
}

impl DiagnosticContext {
    /// Creates a new diagnostic context.
    pub fn new(calls: &[Call], error_message: &str, user_address: Felt) -> Self {
        Self {
            calls: calls.to_vec(),
            error_message: error_message.to_string(),
            user_address,
        }
    }

    /// Finds all calls to a specific contract address.
    pub fn calls_to(&self, contract_address: Felt) -> impl Iterator<Item = &Call> {
        self.calls.iter().filter(move |c| c.to == contract_address)
    }

    /// Finds all calls with a specific selector.
    pub fn calls_with_selector(&self, selector: Felt) -> impl Iterator<Item = &Call> {
        self.calls.iter().filter(move |c| c.selector == selector)
    }

    /// Checks if the error message contains a specific pattern (case-insensitive).
    pub fn error_contains(&self, pattern: &str) -> bool {
        self.error_message.to_lowercase().contains(&pattern.to_lowercase())
    }

    /// Checks if any call targets the given contract.
    pub fn has_call_to(&self, contract_address: Felt) -> bool {
        self.calls.iter().any(|c| c.to == contract_address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper to create a Call with minimal boilerplate
    fn make_call(to: Felt, selector: Felt, calldata: Vec<Felt>) -> Call {
        Call { to, selector, calldata }
    }

    mod calls_to {
        use super::*;

        #[test]
        fn should_return_matching_calls_when_contract_address_matches() {
            // Given
            let target_contract = Felt::from(0x123u64);
            let other_contract = Felt::from(0x456u64);
            let selector = Felt::from(0x789u64);

            let calls = vec![
                make_call(target_contract, selector, vec![]),
                make_call(other_contract, selector, vec![]),
                make_call(target_contract, selector, vec![]),
            ];

            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let matching_calls: Vec<_> = context.calls_to(target_contract).collect();

            // Then
            assert_eq!(matching_calls.len(), 2);
            assert!(matching_calls.iter().all(|c| c.to == target_contract));
        }

        #[test]
        fn should_return_empty_iterator_when_no_calls_match() {
            // Given
            let target_contract = Felt::from(0x123u64);
            let non_existent_contract = Felt::from(0x999u64);

            let calls = vec![make_call(target_contract, Felt::ZERO, vec![])];

            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let matching_calls: Vec<_> = context.calls_to(non_existent_contract).collect();

            // Then
            assert!(matching_calls.is_empty());
        }

        #[test]
        fn should_return_empty_iterator_when_calls_list_is_empty() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let matching_calls: Vec<_> = context.calls_to(Felt::from(0x123u64)).collect();

            // Then
            assert!(matching_calls.is_empty());
        }
    }

    mod calls_with_selector {
        use super::*;

        #[test]
        fn should_return_calls_with_matching_selector() {
            // Given
            let contract = Felt::from(0x123u64);
            let target_selector = Felt::from(0xABCu64);
            let other_selector = Felt::from(0xDEFu64);

            let calls = vec![
                make_call(contract, target_selector, vec![]),
                make_call(contract, other_selector, vec![]),
                make_call(contract, target_selector, vec![]),
            ];

            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let matching_calls: Vec<_> = context.calls_with_selector(target_selector).collect();

            // Then
            assert_eq!(matching_calls.len(), 2);
            assert!(matching_calls.iter().all(|c| c.selector == target_selector));
        }

        #[test]
        fn should_return_empty_iterator_when_selector_not_found() {
            // Given
            let calls = vec![make_call(Felt::from(0x123u64), Felt::from(0xABCu64), vec![])];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let matching_calls: Vec<_> = context.calls_with_selector(Felt::from(0x999u64)).collect();

            // Then
            assert!(matching_calls.is_empty());
        }
    }

    mod error_contains {
        use super::*;

        #[test]
        fn should_return_true_when_pattern_matches_case_insensitive() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "Slippage exceeded: 5%", Felt::ZERO);

            // When & Then
            assert!(context.error_contains("slippage"));
            assert!(context.error_contains("SLIPPAGE"));
            assert!(context.error_contains("Slippage"));
            assert!(context.error_contains("exceeded"));
        }

        #[test]
        fn should_return_false_when_pattern_not_found() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "Slippage exceeded: 5%", Felt::ZERO);

            // When & Then
            assert!(!context.error_contains("balance"));
            assert!(!context.error_contains("insufficient"));
        }

        #[test]
        fn should_handle_empty_error_message() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "", Felt::ZERO);

            // When & Then
            assert!(!context.error_contains("any"));
        }

        #[test]
        fn should_handle_empty_pattern() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "Some error", Felt::ZERO);

            // When & Then
            assert!(context.error_contains(""));
        }
    }

    mod has_call_to {
        use super::*;

        #[test]
        fn should_return_true_when_contract_is_targeted() {
            // Given
            let target_contract = Felt::from(0x123u64);
            let calls = vec![make_call(target_contract, Felt::ZERO, vec![])];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = context.has_call_to(target_contract);

            // Then
            assert!(result);
        }

        #[test]
        fn should_return_false_when_contract_is_not_targeted() {
            // Given
            let target_contract = Felt::from(0x123u64);
            let other_contract = Felt::from(0x456u64);
            let calls = vec![make_call(target_contract, Felt::ZERO, vec![])];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = context.has_call_to(other_contract);

            // Then
            assert!(!result);
        }

        #[test]
        fn should_return_false_when_calls_list_is_empty() {
            // Given
            let calls: Vec<Call> = vec![];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = context.has_call_to(Felt::from(0x123u64));

            // Then
            assert!(!result);
        }

        #[test]
        fn should_return_true_when_contract_appears_multiple_times() {
            // Given
            let target_contract = Felt::from(0x123u64);
            let calls = vec![
                make_call(target_contract, Felt::from(1u64), vec![]),
                make_call(target_contract, Felt::from(2u64), vec![]),
            ];
            let context = DiagnosticContext::new(&calls, "error", Felt::ZERO);

            // When
            let result = context.has_call_to(target_contract);

            // Then
            assert!(result);
        }
    }

    mod new {
        use super::*;

        #[test]
        fn should_store_all_provided_values() {
            // Given
            let calls = vec![make_call(Felt::from(0x123u64), Felt::from(0x456u64), vec![Felt::ONE])];
            let error_message = "test error";
            let user_address = Felt::from(0x789u64);

            // When
            let context = DiagnosticContext::new(&calls, error_message, user_address);

            // Then
            assert_eq!(context.calls.len(), 1);
            assert_eq!(context.error_message, error_message);
            assert_eq!(context.user_address, user_address);
        }
    }
}
