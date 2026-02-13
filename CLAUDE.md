# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the official Starknet paymaster service by AVNU Labs. It's a Rust-based multi-crate workspace that provides paymaster functionality for Starknet transactions, allowing users to pay gas fees with alternative tokens.

## Development Commands

### Build & Test
```bash
# Build the entire workspace
cargo build

# Build specific crate
cargo build -p paymaster-service

# Run tests
cargo test

# Run tests for specific crate
cargo test -p paymaster-starknet

# Format code
cargo fmt

# Run linter
cargo clippy
```

### Running Services

#### Main Service
```bash
# Run the main paymaster service
cargo run -p paymaster-service

# Run with specific configuration
PAYMASTER_CONFIG=config.json cargo run -p paymaster-service
```

#### CLI Tools
```bash
# Run CLI (shows available commands)
cargo run -p paymaster-cli

# Quick setup
cargo run -p paymaster-cli quick-setup

# Deploy relayers
cargo run -p paymaster-cli deploy-relayers

# Check relayer balances
cargo run -p paymaster-cli relayers-balance

# Rebalance relayers
cargo run -p paymaster-cli relayers-rebalance
```

#### Website
```bash
# Navigate to website directory
cd website

# Install dependencies
yarn install

# Run development server
yarn run dev

# Build for production
yarn run build

# Preview production build
yarn run preview
```

### Development Setup
```bash
# Start Redis for development
docker-compose up -d

# This starts Redis on port 6379 as required by the service
```

## Architecture

### Core Components

1. **paymaster-service** - Main service entry point with RPC server
2. **paymaster-rpc** - JSON-RPC API definitions and server implementation
3. **paymaster-relayer** - Relayer management, locking, and rebalancing
4. **paymaster-starknet** - Starknet client abstractions and utilities
5. **paymaster-execution** - Transaction execution and fee estimation
6. **paymaster-prices** - Token price fetching from AVNU
7. **paymaster-sponsoring** - Sponsoring logic and webhook handling
8. **paymaster-common** - Shared utilities, monitoring, and service management
9. **paymaster-cli** - Command-line interface for setup and management
10. **paymaster-client** - Rust client library for the paymaster JSON-RPC API (builder pattern, build+sign+execute flow)
11. **website** - Landing page with documentation links and useful resources

### Key Services

- **RPC Service**: Handles JSON-RPC requests (`paymaster_buildTransaction`, `paymaster_executeTransaction`, etc.)
- **Relayer Manager**: Manages multiple relayers with locking and rebalancing
- **Monitoring Services**: Balance monitoring, transaction monitoring, availability tracking
- **Rebalancing Service**: Automatically rebalances relayer funds using AVNU swaps

### Configuration

The service uses environment variables and configuration files. Key configuration includes:
- Starknet network settings (chain ID, RPC endpoints, fallbacks)
- Relayer configurations (addresses, private keys, balance thresholds)
- Supported tokens and price oracle settings
- Redis/locking configuration
- Monitoring and tracing settings

### Transaction Flow

1. Client calls `paymaster_buildTransaction` to get transaction with paymaster data
2. Transaction is signed by client
3. Client calls `paymaster_executeTransaction` to submit transaction
4. Service locks a relayer, executes transaction, then releases relayer
5. Relayer balances are monitored and rebalanced as needed

### Error Handling

The codebase uses `thiserror` for error handling with comprehensive error types:
- `paymaster_rpc::Error` for RPC-level errors
- `paymaster_starknet::Error` for Starknet interaction errors
- `paymaster_relayer::Error` for relayer management errors

### Monitoring & Observability

- OpenTelemetry integration for tracing
- Prometheus metrics for monitoring
- Structured logging with tracing
- Health checks and availability monitoring

## Development Guidelines

### Code Organization
- Each major component is a separate crate
- Shared utilities in `paymaster-common`
- Testing utilities in each crate's `testing` module
- Configuration structures centralized in each crate's `context` module

### Starknet Integration
- Uses `starknet-rs` for Starknet interactions
- Fallback RPC providers for reliability
- Comprehensive error handling for network issues
- Gas price monitoring and fee estimation

### Builder Pattern
- Builders must use the typestate pattern to enforce required fields at compile time
- Required parameters go in the constructor or in methods that transition state
- Optional parameters work on any state via a generic impl block

### Relayer Management
- Segregated locking to prevent race conditions
- Automatic rebalancing via AVNU swaps
- Balance monitoring and alerting
- Transaction monitoring and retry logic

### Language
- The whole codebase is strictly using English
- New/edited comments must be in English as well
- 
- ## Code Style

Follows rustfmt config: max_width=170, chain_width=80, Unix newlines.

**Don't use section comments**: Avoid comments like `// ============` or `// --- Section ---` to delimit code sections. Use module structure and whitespace instead.

## Tools & Plugins

- **rust-analyzer-lsp**: Use this MCP plugin for Rust code analysis
- **plugin:context7:context7**: Use for up-to-date documentation on libraries

## Testing Guidelines

Most crates include comprehensive test suites. Key testing utilities:
- Mock implementations for external dependencies
- Test transactions and accounts in `paymaster-starknet/testing`
- Integration tests for RPC endpoints
- Relayer lock testing with mock coordination layers

### Test Patterns

- Unit tests: `#[cfg(test)] mod tests` at bottom of file
- Integration tests: Use testcontainers with `setup_mongo()`
- Naming: `should_<action>_when_<condition>`
- Structure: Given/When/Then comments
- Nested modules per function: `mod function_name { #[test] fn should_... }`

### Test Pragmatism

Write tests that provide real value. Avoid over-testing and redundant coverage:

- **Test each behavior once**: If a helper function has 2 behaviors (success/error), test those 2 cases in unit tests for that function
- **Don't re-test dependencies**: If function B calls function A, only test B's happy path with A succeeding. A's error cases are already covered by A's own tests
- **Avoid trivial tests**: Don't test obvious things like "constructor sets fields" or "getter returns value"
- **Focus on boundaries**: Test edge cases at the lowest level where they occur, not at every layer

Example - What NOT to do:
```rust
// parse_signature already has tests for empty input, invalid hex, etc.
// Don't re-test those cases in verify_signature_endpoint!

// ❌ BAD: Re-testing parse_signature errors through the endpoint
#[case::empty_signature(json!({ ..., "signature": [] }))]  // Already tested in parse_signature
#[case::invalid_hex(json!({ ..., "signature": ["invalid"] }))]  // Already tested

// ✅ GOOD: Test only verify_signature_endpoint's own logic
#[case::address_mismatch(...)]  // Endpoint's own validation
#[case::nonce_not_found(...)]   // Endpoint's own behavior
```

**Rule of thumb**: If a test failure would point you to a dependency's code rather than the function being tested, the test is redundant.

### rstest Parameterized Tests

Use `rstest` for parameterized tests, grouped by outcome:
- **Split by success/error**: Don't mix success and error cases in the same function
- **Split by HTTP status**: Group endpoint tests by expected status code (400, 401, etc.)
- **Payload in `#[case]`**: Put the full input data directly in the case attribute for visibility

Example - Unit tests split by success/error:
```rust
#[rstest]
#[case::valid_hex(vec!["0x123", "0x456"], 2)]
#[case::single_element(vec!["0x123"], 1)]
fn should_parse_valid_signature_when(#[case] input: Vec<&str>, #[case] expected_len: usize) {
    let result = parse_signature(&input.into_iter().map(String::from).collect::<Vec<_>>());
    assert_eq!(result.unwrap().len(), expected_len);
}

#[rstest]
#[case::empty(vec![])]
#[case::decimal_rejected(vec!["123", "456"])]
fn should_reject_invalid_signature_when(#[case] input: Vec<&str>) {
    let result = parse_signature(&input.into_iter().map(String::from).collect::<Vec<_>>());
    assert!(result.is_err());
}
```

Example - HTTP endpoint tests grouped by status code:
```rust
#[rstest]
#[case::invalid_wallet_address(json!({ "wallet_address": "invalid", ... }))]
#[case::invalid_message_address(json!({ "wallet_address": "0x...", "message": { "address": "invalid" }, ... }))]
#[actix_rt::test]
async fn should_return_400_when(#[case] payload: serde_json::Value) {
    // ... test returning 400
}

#[rstest]
#[case::address_mismatch(json!({ ... }))]
#[actix_rt::test]
async fn should_return_401_when(#[case] payload: serde_json::Value) {
    // ... test returning 401
}
```

**Note**: Tests requiring specific setup (database operations before request) should remain as individual functions.

## CLAUDE.md Maintenance

**REQUIRED**: Update this file when making significant architecture changes:
- Document major new features
- Update environment variables when adding new ones
- Keep the "Progress Tracker" section up to date
- Update API structure when adding new endpoints
