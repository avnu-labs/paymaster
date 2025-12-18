use std::io::{self, Write};

use clap::Args;
use paymaster_service::core::context::configuration::Configuration as ServiceConfiguration;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::{denormalize_felt, normalize_felt};
use paymaster_starknet::transaction::Calls;
use paymaster_starknet::{Client, Configuration, StarknetAccountConfiguration};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::Felt;
use tracing::info;

use crate::command::relayer::build::RelayerDeployment;
use crate::constants::DEFAULT_MAX_CHECK_STATUS_ATTEMPTS;
use crate::core::starknet::transaction::status::wait_for_transaction_success;
use crate::core::starknet::transaction::transfer::Transfer;
use crate::core::Error;
use crate::validation::assert_strk_balance;

#[derive(Args, Clone)]
pub struct RelayersDeployCommandParameters {
    #[clap(long)]
    pub master_address: Felt,

    #[clap(long)]
    pub master_pk: Felt,

    #[clap(long)]
    pub num_relayers: usize,

    #[clap(long, default_value_t = 0.0)]
    pub fund: f64,

    #[clap(long)]
    pub profile: String,

    #[clap(long, default_value_t = DEFAULT_MAX_CHECK_STATUS_ATTEMPTS)]
    pub max_check_status_attempts: usize,

    #[clap(short, long, help = "Force deployment without user confirmation")]
    pub force: bool,
}

pub async fn command_relayers_deploy(params: RelayersDeployCommandParameters) -> Result<(), Error> {
    info!("🚀 Starting relayers deployment for profile: {}", params.profile);

    // Load the configuration
    let mut configuration = ServiceConfiguration::from_file(&params.profile).unwrap();
    let chain_id = configuration.starknet.chain_id;
    let rpc_url = configuration.starknet.endpoint.clone();

    let fund_gas_tank_in_fri = normalize_felt(params.fund, 18);
    let num_relayers = params.num_relayers;

    // Print the parameters to the user
    info!("Using chain-id: {}", chain_id.as_identifier());
    info!("Using RPC URL: {}", rpc_url);
    info!("Nbr of new relayers: {}", num_relayers);
    info!("Fund gas tank with: {} STRK", denormalize_felt(fund_gas_tank_in_fri, 18));
    info!("Profile path: {}", params.profile);

    // Initialize the Starknet client
    let starknet = Client::new(&Configuration {
        endpoint: rpc_url.clone(),
        chain_id,
        fallbacks: vec![],
        timeout: configuration.starknet.timeout,
    });

    // Assert the balance of master is greater than the amount of STRK needed for the deployment
    // If not, stop the deployment execution
    assert_strk_balance(&starknet, params.master_address, fund_gas_tank_in_fri)
        .await
        .unwrap();

    // Ask user for confirmation before proceeding (unless force flag is used)
    if !params.force {
        print!(
            "Do you want to proceed with the deployment? This will transfer {} STRK tokens to your new relayers. (y/N): ",
            denormalize_felt(fund_gas_tank_in_fri, 18)
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| Error::Execution(format!("Failed to read user input: {}", e)))?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            info!("Deployment cancelled by user.");
            return Ok(());
        }
    }

    info!("Proceeding with contracts deployment...");

    // Prepare multicall
    let account = starknet.initialize_account(&StarknetAccountConfiguration {
        address: params.master_address,
        private_key: params.master_pk,
    });

    let relayers_deployment = RelayerDeployment::build_many(
        &starknet,
        configuration.forwarder,
        configuration.relayers.private_key,
        num_relayers,
        Felt::ZERO, // We don't fund the relayers with STRK, we load the gas tank instead
    )
    .await?;

    let fund_gas_tank_transfer = Transfer {
        recipient: configuration.gas_tank.address,
        token: Token::STRK_ADDRESS,
        amount: fund_gas_tank_in_fri,
    };

    let mut multicall = Calls::empty();
    multicall.merge(&relayers_deployment.calls);
    if fund_gas_tank_in_fri != Felt::ZERO {
        multicall.push(fund_gas_tank_transfer.as_call());
    }

    // Execute multicall
    let nonce = account.get_nonce().await.unwrap();
    let result = multicall.execute(&account, nonce).await.unwrap();

    // Wait for tx to be executed
    wait_for_transaction_success(&starknet, result.transaction_hash, params.max_check_status_attempts).await?;

    /********* New relayers are deployed *********/
    info!(
        "✅ {} new relayers successfully deployed, tx hash: {}",
        num_relayers,
        result.transaction_hash.to_fixed_hex_string()
    );

    // Merge new relayers addresses with existing ones & write configuration
    let mut all_relayers_addresses = configuration.relayers.addresses.clone();
    all_relayers_addresses.extend(relayers_deployment.addresses.clone());
    configuration.relayers.addresses = all_relayers_addresses;
    let _ = configuration.write_to_file(&params.profile);

    info!(
        "📝 Configuration file is updated with {} total relayers, see {}",
        configuration.relayers.addresses.len(),
        params.profile
    );

    Ok(())
}
