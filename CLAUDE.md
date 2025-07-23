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

### Testing

Most crates include comprehensive test suites. Key testing utilities:
- Mock implementations for external dependencies
- Test transactions and accounts in `paymaster-starknet/testing`
- Integration tests for RPC endpoints
- Relayer lock testing with mock coordination layers

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

### Relayer Management
- Segregated locking to prevent race conditions
- Automatic rebalancing via AVNU swaps
- Balance monitoring and alerting
- Transaction monitoring and retry logic

### Language
- The whole codebase is strictly using English
- New/edited comments must be in English as well