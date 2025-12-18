use std::io::stdin;
use std::time::Duration;

use clap::Args;
use paymaster_service::core::context::configuration::Configuration as ServiceConfiguration;
use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::{Calls, TimeBounds};
use paymaster_starknet::{Client, Configuration, StarknetAccountConfiguration};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Felt;
use tracing::{info, warn};

use crate::core::starknet::transaction::transfer::Transfer;
use crate::core::Error;

#[derive(Args, Clone)]
pub struct EmptyPaymasterParameters {
    #[clap(long)]
    pub master_address: Felt,

    #[clap(long)]
    pub master_pk: Felt,

    #[clap(long)]
    pub profile: String,

    #[clap(short, long, help = "Force emptying without user confirmation")]
    pub force: bool,
}

/// Core empty paymaster logic that can be reused by both CLI and integration tests
pub async fn empty_paymaster_core(
    starknet: &Client,
    configuration: &ServiceConfiguration,
    master_address: Felt,
    master_pk: Felt,
    skip_confirmation: bool,
) -> Result<Felt, Error> {
    info!("🧹 Emptying paymaster (gas tank + relayers + estimate account) to master account...");

    // Warn user that this will empty the paymaster and all relayers will be deactivated (unless skip_confirmation is true)
    if !skip_confirmation {
        warn!("⚠️ This will empty the paymaster and all relayers will be deactivated");
        warn!("⚠️ Are you sure you want to proceed? (y/N): ");
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        if input.trim().to_lowercase() != "y" {
            return Ok(Felt::ZERO);
        }
    }

    let master_account = starknet.initialize_account(&StarknetAccountConfiguration {
        address: master_address,
        private_key: master_pk,
    });

    // Create a transfer call from each relayer
    let strk = Token::STRK_ADDRESS;
    let caller = master_address;

    let mut relayers_empty_calls_from_outside = Vec::new();
    for address in &configuration.relayers.addresses {
        let mut relayers_empty_transfer = Vec::new();
        let balance = starknet.fetch_balance(strk, *address).await.unwrap();
        if balance > Felt::ZERO {
            let transfer_call = Transfer {
                token: strk,
                recipient: caller,
                amount: balance,
            };
            let call = transfer_call.as_call();
            relayers_empty_transfer.push(call);
            let relayer_empty_call = Calls::new(relayers_empty_transfer);
            let relayer_pk = configuration.relayers.private_key;
            let relayer_account = starknet.initialize_account(&StarknetAccountConfiguration {
                address: *address,
                private_key: relayer_pk,
            });
            let outside_call = relayer_empty_call.as_execute_from_outside_call(caller, relayer_account, relayer_pk, TimeBounds::valid_for(Duration::from_secs(3600)));
            relayers_empty_calls_from_outside.push(outside_call);
        }
    }

    // Create a transfer call for each of supported_tokens from the gas tank
    let gas_tank_account = starknet.initialize_account(&StarknetAccountConfiguration {
        address: configuration.gas_tank.address,
        private_key: configuration.gas_tank.private_key,
    });

    let mut gas_tank_empty_tokens_transfer = Vec::new();
    for token in &configuration.supported_tokens {
        let balance = starknet.fetch_balance(*token, configuration.gas_tank.address).await.unwrap();
        if balance > Felt::ZERO {
            let transfer_call = Transfer {
                token: *token,
                recipient: caller,
                amount: balance,
            };
            gas_tank_empty_tokens_transfer.push(transfer_call.as_call());
        }
    }
    // Create a transfer call of STRK token from the gas tank
    let gas_tank_strk_balance = starknet.fetch_balance(strk, configuration.gas_tank.address).await.unwrap();
    if gas_tank_strk_balance > Felt::ZERO {
        let gas_tank_strk_empty_call = Transfer {
            token: strk,
            recipient: caller,
            amount: gas_tank_strk_balance,
        }
        .as_call();
        gas_tank_empty_tokens_transfer.push(gas_tank_strk_empty_call);
    }

    // Create a transfer call of STRK token from the estimate account
    let estimate_account_account = starknet.initialize_account(&StarknetAccountConfiguration {
        address: configuration.estimate_account.address,
        private_key: configuration.estimate_account.private_key,
    });

    let mut estimate_account_empty_tokens_transfer = Vec::new();
    let estimate_account_balance = starknet
        .fetch_balance(strk, configuration.estimate_account.address)
        .await
        .unwrap();

    if estimate_account_balance > Felt::ZERO {
        let estimate_account_strk_empty_call = Transfer {
            token: strk,
            recipient: caller,
            amount: estimate_account_balance,
        }
        .as_call();
        estimate_account_empty_tokens_transfer.push(estimate_account_strk_empty_call);
    }

    // Only execute if there are calls to make
    if !relayers_empty_calls_from_outside.is_empty() || !gas_tank_empty_tokens_transfer.is_empty() || !estimate_account_empty_tokens_transfer.is_empty() {
        let mut all_calls = relayers_empty_calls_from_outside;

        if !gas_tank_empty_tokens_transfer.is_empty() {
            let gas_tank_empty_tokens_calls = Calls::new(gas_tank_empty_tokens_transfer);
            let gas_tank_empty_tokens_calls_from_outside = gas_tank_empty_tokens_calls.as_execute_from_outside_call(
                caller,
                gas_tank_account.clone(),
                configuration.gas_tank.private_key,
                TimeBounds::valid_for(Duration::from_secs(3600)),
            );
            all_calls.push(gas_tank_empty_tokens_calls_from_outside);
        }

        if !estimate_account_empty_tokens_transfer.is_empty() {
            let estimate_account_empty_tokens_calls = Calls::new(estimate_account_empty_tokens_transfer);
            let estimate_account_empty_tokens_calls_from_outside = estimate_account_empty_tokens_calls.as_execute_from_outside_call(
                caller,
                estimate_account_account.clone(),
                configuration.estimate_account.private_key,
                TimeBounds::valid_for(Duration::from_secs(3600)),
            );
            all_calls.push(estimate_account_empty_tokens_calls_from_outside);
        }

        let multicall = Calls::new(all_calls);
        let nonce = master_account.get_nonce().await.unwrap();
        let result = multicall.execute(&master_account, nonce).await.unwrap();

        let tx_hash = result.transaction_hash;
        info!("✅ Paymaster emptied successfully, tx hash: {}", tx_hash.to_fixed_hex_string());
        Ok(tx_hash)
    } else {
        info!("ℹ️ No balances to empty - paymaster is already empty");
        Ok(Felt::ZERO)
    }
}

/// CLI wrapper that uses the core empty paymaster logic
pub async fn command_empty_paymaster(params: EmptyPaymasterParameters) -> Result<(), Error> {
    info!("Emptying paymaster for profile: {}", params.profile);

    // Load the configuration from the profile
    let configuration = ServiceConfiguration::from_file(&params.profile).unwrap();
    let chain_id = configuration.starknet.chain_id.clone();
    let rpc_url = configuration.starknet.endpoint.clone();

    // Print the parameters to the user
    info!("Using chain-id: {}", chain_id.as_identifier());
    info!("Using RPC URL: {}", rpc_url);
    info!("Profile path: {}", params.profile);

    let starknet = Client::new(&Configuration {
        endpoint: rpc_url,
        chain_id: chain_id.clone(),
        fallbacks: vec![],
        timeout: configuration.starknet.timeout,
    });

    empty_paymaster_core(&starknet, &configuration, params.master_address, params.master_pk, params.force).await?;

    Ok(())
}
