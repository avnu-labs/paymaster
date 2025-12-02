//! Contract-specific metadata extractors.
//!
//! This module provides extractors for known contracts on Starknet.
//!
//! This module contains implementations of [`CallMetadataExtractor`] for various
//! protocols and contracts. Each extractor focuses on a specific contract and
//! extracts meaningful diagnostic information from failed transactions.
//!
//! # Adding a new extractor
//!
//! 1. Create a new file in this directory (e.g., `jediswap.rs`)
//! 2. Implement the [`CallMetadataExtractor`] trait
//! 3. Re-export the extractor from this module
//! 4. Register it with the [`DiagnosticService`] during initialization
//!
//! # Example
//!
//! ```ignore
//! // In extractors/myprotocol.rs
//! pub struct MyProtocolExtractor { ... }
//!
//! impl CallMetadataExtractor for MyProtocolExtractor { ... }
//!
//! // In extractors/mod.rs
//! mod myprotocol;
//! pub use myprotocol::MyProtocolExtractor;
//!
//! // DiagnosticService::new(chain_id) automatically registers known extractors.
//! // Custom extractors can be added internally in the service module.
//! ```

mod avnu;

pub use avnu::{AvnuExtractor, AVNU_EXCHANGE_ADDRESS};
