# Gasless Contracts

This directory contains contracts that are used to provide the paymaster service.

It declares a simple Forwarder contract. This one exposes four entrypoints:

- `execute`: It verifies if the caller is whitelisted (only whitelisted relayers can execute user's calls), executes user's calls and collect user's gas tokens
- `execute_sponsored`: It does the same as `execute` but it doesn't collect user's gas tokens
- `execute_private_sponsored`: It executes a list of calls with sponsor metadata, collects pool fees if any, and emits a sponsored transaction event. Used for sponsored privacy transactions
- `execute_private`: It executes a list of calls, verifies the forwarder received the expected gas token amount during execution, and transfers it to the gas fees recipient. Used for gasless private transactions

Here is the interface of the Forwarder contract:

```cairo
#[starknet::interface]
trait IForwarder<TContractState> {
    fn get_gas_fees_recipient(self: @TContractState) -> ContractAddress;
    fn set_gas_fees_recipient(ref self: TContractState, gas_fees_recipient: ContractAddress) -> bool;
    fn execute(
        ref self: TContractState,
        account_address: ContractAddress,
        entrypoint: felt252,
        calldata: Array<felt252>,
        gas_token_address: ContractAddress,
        gas_amount: u256,
    ) -> bool;
    fn execute_sponsored(
        ref self: TContractState,
        account_address: ContractAddress,
        entrypoint: felt252,
        calldata: Array<felt252>,
        sponsor_metadata: Array<felt252>,
    ) -> bool;
    fn execute_private(
        ref self: TContractState,
        calls: Array<Call>,
        gas_token_address: ContractAddress,
        gas_amount: u256,
    ) -> bool;
    fn execute_private_sponsored(
        ref self: TContractState,
        calls: Array<Call>,
        gas_token_address: ContractAddress,
        gas_amount: u256,
        sponsor_metadata: Array<felt252>,
    ) -> bool;
}
```

## Getting Started

This repository is using [Scarb](https://docs.swmansion.com/scarb/) to install, test, build contracts

```shell
# Format
scarb fmt

# Run the tests
scarb test

# Build contracts
scarb build
```
