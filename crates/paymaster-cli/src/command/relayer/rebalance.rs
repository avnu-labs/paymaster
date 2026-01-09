use std::io::{stdin, stdout, Write};
use std::time::Duration;

use clap::Args;
use log::{error, info};
use paymaster_common::service::Service;
use paymaster_relayer::{Context, RelayerManagerConfiguration, RelayerRebalancingService};
use paymaster_service::core::context::configuration::Configuration as ServiceConfiguration;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::{denormalize_felt, normalize_felt};
use paymaster_starknet::transaction::{Calls, TimeBounds};
use paymaster_starknet::{Client, Configuration};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;

use crate::core::starknet::transaction::transfer::Transfer;
use crate::core::Error;
use crate::validation::assert_strk_balance;

#[derive(Args, Clone)]
pub struct RelayersRebalanceCommandParameters {
    #[clap(long)]
    pub master_address: Felt,

    #[clap(long)]
    pub master_pk: Felt,

    #[clap(long, default_value_t = 0.0)]
    pub fund: f64,

    #[clap(long, default_value = "false")]
    pub swap: bool,

    #[clap(long)]
    pub profile: String,

    #[clap(short, long, help = "Force rebalancing without user confirmation")]
    pub force: bool,
}

pub async fn command_relayers_rebalance(params: RelayersRebalanceCommandParameters) -> Result<(), Error> {
    info!("🔄 Starting relayers rebalancing for profile: {}", params.profile);

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

    // How much STRK to refund the gas tank with from the master account
    let additional_strk_balance = normalize_felt(params.fund, 18);

    // Assert the balance of master is greater than the amount of STRK needed for the refund
    assert_strk_balance(&starknet, params.master_address, additional_strk_balance)
        .await
        .unwrap();

    // Initialize the gas tank account
    let gas_tank = starknet.initialize_account(&configuration.gas_tank);
    let gas_tank_private_key = configuration.gas_tank.private_key;

    // Initialize the rebalancing service
    let rebalancing_service = RelayerRebalancingService::new(Context::new(RelayerManagerConfiguration {
        starknet: configuration.starknet.clone(),
        gas_tank: configuration.gas_tank.clone(),
        relayers: configuration.relayers.clone(),
        supported_tokens: configuration.supported_tokens.clone(),
        price: configuration.clone().into(),
    }))
    .await;

    // If swap is enabled, swap the supported tokens balance to STRK (in gas tank)
    let (swap_calls, swap_resulted_strk_balance) = if params.swap {
        info!("Try to swap supported tokens to STRK");
        rebalancing_service.swap_to_strk_calls().await.unwrap()
    } else {
        (Calls::new(vec![]), Felt::ZERO) // Empty calls if swap is not enabled
    };

    // Try to rebalance the relayers (in gas tank)
    let rebalancing_calls = match rebalancing_service
        .try_rebalance(additional_strk_balance + swap_resulted_strk_balance)
        .await
    {
        Ok(calls) => calls,
        Err(e) => {
            error!("Failed to rebalance: {:?}", e);
            return Ok(());
        },
    };

    let mut refund_call = None;
    // Refund the gas tank if necessary (transfer STRK to the gas tank from the master account)
    if additional_strk_balance > Felt::ZERO {
        // Ask user for confirmation before proceeding (unless force flag is used)
        if !params.force {
            print!(
                "Do you want to proceed with the rebalance? This will transfer an additional {} STRK tokens to the gas tank. (y/N): ",
                denormalize_felt(additional_strk_balance, 18)
            );
            stdout().flush().unwrap();

            let mut input = String::new();
            stdin()
                .read_line(&mut input)
                .map_err(|e| Error::Execution(format!("Failed to read user input: {}", e)))?;

            let input = input.trim().to_lowercase();
            if input != "y" && input != "yes" {
                info!("Deployment cancelled by user.");
                return Ok(());
            }
        }

        refund_call = Some(
            Transfer {
                token: Token::STRK_ADDRESS,
                recipient: gas_tank.address(),
                amount: additional_strk_balance,
            }
            .as_call(),
        )
    }

    // Merge all calls that need to be executed
    let mut refilling_calls_from_gas_tank = Calls::new(vec![]);
    refilling_calls_from_gas_tank.merge(&swap_calls);
    refilling_calls_from_gas_tank.merge(&rebalancing_calls);

    if refilling_calls_from_gas_tank.is_empty() && refund_call.is_none() {
        info!("No calls to execute");
        return Ok(());
    }

    // Create the execute_from_outside call to call rebalancing on the gas tank account
    let execute_from_outside_call = refilling_calls_from_gas_tank.as_execute_from_outside_call(
        params.master_address,
        gas_tank,
        gas_tank_private_key,
        TimeBounds::valid_for(Duration::from_secs(3600)),
    );

    // Create the final transaction that calls execute_from_outside on the gas tank (+ refund first if necessary)
    let multi_calls = if let Some(refund_call) = refund_call {
        Calls::new(vec![refund_call, execute_from_outside_call])
    } else {
        Calls::new(vec![execute_from_outside_call])
    };

    // Execute the transaction using the master account
    let master_account = starknet.initialize_account(&paymaster_starknet::StarknetAccountConfiguration {
        address: params.master_address,
        private_key: params.master_pk,
    });

    let master_account_nonce = master_account.get_nonce().await.unwrap();

    let estimated_calls = match multi_calls.estimate(&master_account, None).await {
        Ok(calls) => calls,
        Err(e) => {
            error!("❌ Failed to estimate calls: {:?}", e);
            return Ok(());
        },
    };

    let result = estimated_calls.execute(&master_account, master_account_nonce).await.unwrap();
    info!(
        "✅ Rebalancing transaction executed successfully, tx hash: {}",
        result.transaction_hash.to_fixed_hex_string()
    );

    Ok(())
}
