//! Diagnostics module for extracting contextual information from transaction errors.
//!
//! This module provides a framework for analyzing transaction simulation failures
//! and extracting meaningful metadata from the error context. It uses a registry
//! of extractors that can identify specific contract errors (e.g., AVNU slippage)
//! and produce structured diagnostic information for logging and monitoring.
//!
//! # Architecture
//!
//! The module follows the Strategy pattern with a simple registry:
//!
//! - [`DiagnosticContext`]: Contains the transaction calls and error information
//! - [`CallMetadataExtractor`]: Trait for implementing contract-specific extractors
//! - [`DiagnosticClient`]: Registry that manages extractors and orchestrates analysis
//! - [`CallDiagnostic`]: The output containing extracted metadata for logging
//!
//! # Usage
//!
//! ```ignore
//! // The DiagnosticService is typically created via the execution Client
//! // which automatically configures it with the AVNU extractor for the chain.
//! let service = DiagnosticService::new(chain_id);
//!
//! let context = DiagnosticContext::new(&calls, &error_message, user_address);
//! if let Some(diagnostic) = service.analyze(&context).await {
//!     tracing::warn!(
//!         contract = diagnostic.contract_name,
//!         category = ?diagnostic.error_category,
//!         "Transaction failed with context"
//!     );
//! }
//! ```

mod client;
mod context;
mod extractor;

pub mod extractors;

pub use client::DiagnosticClient;
pub use context::DiagnosticContext;
pub use extractor::{CallDiagnostic, CallMetadataExtractor, DiagnosticMetric, DiagnosticValue};
