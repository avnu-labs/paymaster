use clap::Args;
use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::service::Error as ServiceError;
use paymaster_common::task;
use paymaster_service::core::context::configuration::Configuration as ServiceConfiguration;
use paymaster_starknet::constants::Token;
use paymaster_starknet::{Client, Configuration};
use starknet::core::types::Felt;
use tracing::info;

use crate::command::balance::utils::display_table;
use crate::core::Error;

mod utils;

pub struct BalanceResult {
    pub address: Felt,
    pub balance: Felt,
}

#[derive(Args, Clone)]
pub struct BalancesCommandParameters {
    #[clap(long)]
    pub profile: String,
}

async fn compute_table_for_accounts(account_name: &str, starknet: &Client, accounts: Vec<Felt>) {
    // Fetch the accounts balance concurrently
    let nb_accounts = accounts.len();
    let mut executor = ConcurrentExecutor::new(starknet.clone(), nb_accounts);
    for &account in &accounts {
        executor.register(task!(|env| {
            let balance = env.fetch_balance(Token::strk(env.chain_id()).address, account).await?;

            Ok::<BalanceResult, paymaster_starknet::Error>(BalanceResult { address: account, balance })
        }));
    }

    // Compute results
    let results = executor
        .execute()
        .await
        .map_err(|e| ServiceError::new(&format!("Failed to fetch accounts balances: {}", e)))
        .unwrap();

    // Display results as a table
    display_table(&results, account_name);
}

pub async fn command_balances(params: BalancesCommandParameters) -> Result<(), Error> {
    info!("💰 Fetching relayers balance for profile: {}", params.profile);

    // Load the configuration from the profile
    let configuration = ServiceConfiguration::from_file(&params.profile).unwrap();
    let chain_id = configuration.starknet.chain_id;
    let rpc_url = configuration.starknet.endpoint;

    // Print the parameters to the user
    info!("Using chain-id: {}", chain_id.as_identifier());
    info!("Using RPC URL: {}", rpc_url);
    info!("Profile path: {}", params.profile);

    let starknet = Client::new(&Configuration {
        endpoint: rpc_url,
        chain_id,
        fallbacks: vec![],
        timeout: 10,
    });

    // Display relayers balances
    compute_table_for_accounts("Relayer", &starknet, configuration.relayers.addresses).await;

    // Display gas tank balance
    compute_table_for_accounts("Gas Tank", &starknet, vec![configuration.gas_tank.address]).await;

    // Display estimate account balance
    compute_table_for_accounts("Estimate", &starknet, vec![configuration.estimate_account.address]).await;

    Ok(())
}
