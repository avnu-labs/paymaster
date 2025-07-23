# Contributing to Paymaster

Thank you for your interest in contributing to the Paymaster project! This guide will help you get started.

## Getting Started

### Prerequisites

- Rust (>=1.85.0)
- Docker (for running tests, or the service with containers)
- Git

### Setting up the Development Environment

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/avnu-labs/paymaster && cd paymaster
   ```

2. **Install dependencies**
   ```bash
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

## Project Structure

The project is organized as a Rust workspace with the following main crates:

- `paymaster-service/` - Main service binary
- `paymaster-rpc/` - RPC server and endpoints
- `paymaster-execution/` - Transaction execution logic
- `paymaster-sponsoring/` - Authentication and sponsoring mechanism
- `paymaster-starknet/` - Starknet client and utilities
- `paymaster-relayer/` - Relayer management, gas conversion, auto rebalancing
- `paymaster-prices/` - Price oracle integration
- `paymaster-common/` - Shared utilities
- `paymaster-cli/` - Command-line interface tool

## Making Changes

### Code Style

- Follow standard Rust formatting with `cargo fmt`
- Ensure all tests pass with `cargo test`
- Follow general Rust best practices, reuse components from `paymaster-starknet`, `paymaster-common` crates instead of reinventing the wheel

### Testing

- Please write unit tests for new functionality
- Some tests require Docker containers eg. Redis

### Pull Request Process

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Write clear, concise commit messages
   - Add tests for new functionality
   - Give credit to others if your work derives from other projects

3. **Test your changes**
   ```bash
   cargo test
   cargo fmt --check
   ```

4. **Submit a pull request**
   - Provide a clear description of the changes
   - Reference any related issues - feature requests or bug reports
   - Ensure CI checks pass

## Some More Development Guidelines

### Error Handling

- Use the `Error` types defined in each crate
- Prefer `Result<T, Error>` for fallible operations following Rust best practices

### Logging and Metrics

- Use the `tracing` crate for structured logging
- Add metrics using the `metric!` macro from `paymaster-common`
- Follow the existing patterns for instrumentation - create your own otel collector for testing if required

### Configuration

- Configuration is handled through JSON files, command line arguments and environment variables
- See `resources/specification/configuration.json` for the structure

### Contributions Related to Spelling and Grammar

At this time, we will not be accepting contributions that only fix spelling or grammatical errors in documentation, code or elsewhere.

## Reporting Issues

When reporting bugs or requesting features:

- Use the GitHub issue templates
- Provide clear reproduction steps
- Include relevant logs and error messages
- Give clear reasoning, and as much detail as possible, when requesting a new feature
- For any security related issue, please contact maintainers before opening an issue

## Getting Help / Troubleshooting

If you need help or have questions:

- Check existing issues and discussions
- Ask questions in pull request comments
- Review the code documentation
- Get in touch with the maintainers on the [telegram channel](https://t.me/avnu_developers)

## Contributing in other ways

Answer questions on the telegram channel, github issues - every question answered creates value for the participants of the ecosystem and is much appreciated.
Review other PRs if you can, and use Paymaster in real-world projects if possible - real usage will show us areas of friction and improvement.

Thank you for contributing to the Paymaster.