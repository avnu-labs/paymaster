use std::collections::HashSet;
use std::io::{self, Write};
use std::str::FromStr;
use std::time::Duration;

use clap::Args;
use paymaster_common::service::Service;
use paymaster_prices::avnu::AVNUPriceClientConfiguration;
use paymaster_prices::Configuration as PriceConfiguration;
use paymaster_relayer::rebalancing::{OptionalRebalancingConfiguration, RebalancingConfiguration};
use paymaster_relayer::swap::client::SwapClientConfiguration;
use paymaster_relayer::swap::{SwapClientConfigurator, SwapConfiguration};
use paymaster_relayer::{Context as RelayerContext, RelayerManagerConfiguration, RelayerRebalancingService, RelayersConfiguration};
use paymaster_rpc::RPCConfiguration;
use paymaster_service::core::context::configuration::{Configuration as ServiceConfiguration, VerbosityConfiguration};
use paymaster_starknet::constants::{Endpoint, Token};
use paymaster_starknet::math::{format_units, parse_units};
use paymaster_starknet::transaction::{Calls, TimeBounds};
use paymaster_starknet::{ChainID, Client, Configuration as StarknetConfiguration, Configuration, StarknetAccountConfiguration};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{Call, Felt};
use starknet::signers::SigningKey;
use tracing::info;

use crate::command::forwarder::build::ForwarderDeployment;
use crate::command::gas_tank::build::GasTankDeployment;
use crate::command::relayer::build::RelayerDeployment;
use crate::constants::{
    DEFAULT_INITIAL_ESTIMATE_ACCOUNT_FUND_AMOUNT, DEFAULT_INITIAL_GAS_TANK_FUND_AMOUNT, DEFAULT_MAX_CHECK_STATUS_ATTEMPTS, DEFAULT_MAX_FEE_MULTIPLIER,
    DEFAULT_MAX_PRICE_IMPACT, DEFAULT_MIN_RELAYER_BALANCE, DEFAULT_MIN_SWAP_SELL_AMOUNT, DEFAULT_PROVIDER_FEE_OVERHEAD, DEFAULT_REBALANCING_CHECK_INTERVAL,
    DEFAULT_RELAYERS_LOCK_MODE, DEFAULT_RELAYERS_NUM, DEFAULT_RELAYERS_REBALANCE_TRIGGER_AMOUNT, DEFAULT_RPC_PORT, DEFAULT_SPONSORING_MODE, DEFAULT_STARKNET_TIMEOUT,
    DEFAULT_SWAP_INTERVAL, DEFAULT_SWAP_SLIPPAGE, DEFAULT_VERBOSITY,
};
use crate::core::starknet::transaction::status::wait_for_transaction_success;
use crate::core::Error;
use crate::validation::{assert_rebalancing_configuration, assert_strk_balance};

#[derive(Args, Clone)]
pub struct SetupParameters {
    #[clap(long)]
    pub rpc_url: Option<String>,

    #[clap(long, default_value_t = DEFAULT_STARKNET_TIMEOUT)]
    pub rpc_timeout: u64,

    #[clap(long, default_value_t = DEFAULT_RPC_PORT)]
    pub rpc_port: u64,

    #[clap(long)]
    pub chain_id: String,

    #[clap(long)]
    pub master_address: Felt,

    #[clap(long)]
    pub master_pk: Felt,

    #[clap(long, default_value_t = DEFAULT_RELAYERS_NUM)]
    pub num_relayers: usize,

    #[clap(long, default_value_t = DEFAULT_INITIAL_GAS_TANK_FUND_AMOUNT)]
    pub fund: f64,

    #[clap(long, default_value_t = DEFAULT_INITIAL_ESTIMATE_ACCOUNT_FUND_AMOUNT)]
    pub estimate_account_fund: f64,

    #[clap(long, default_value = "default.json")]
    pub profile: String,

    #[clap(long, default_value_t = DEFAULT_MAX_CHECK_STATUS_ATTEMPTS)]
    pub max_check_status_attempts: usize,

    #[clap(long, default_value_t = DEFAULT_MIN_SWAP_SELL_AMOUNT)]
    pub min_swap_sell_amount: f64,

    #[clap(long, default_value_t = DEFAULT_MAX_FEE_MULTIPLIER)]
    pub max_fee_multiplier: f32,

    #[clap(long, default_value_t = DEFAULT_PROVIDER_FEE_OVERHEAD)]
    pub fee_overhead: f32,

    #[clap(long, default_value_t = DEFAULT_MIN_RELAYER_BALANCE)]
    pub min_relayer_balance: f64,

    #[clap(long, default_value_t = DEFAULT_REBALANCING_CHECK_INTERVAL)]
    pub rebalancing_check_interval: u64,

    #[clap(long, default_value_t = DEFAULT_RELAYERS_REBALANCE_TRIGGER_AMOUNT)]
    pub rebalancing_trigger_balance: f64,

    #[clap(long, default_value_t = DEFAULT_SWAP_SLIPPAGE)]
    pub swap_slippage: f64,

    #[clap(long, default_value_t = DEFAULT_SWAP_INTERVAL)]
    pub swap_interval: u64,

    #[clap(long, default_value_t = DEFAULT_MAX_PRICE_IMPACT)]
    pub max_price_impact: f64,

    #[clap(long, default_value = DEFAULT_VERBOSITY)]
    pub verbosity: String,
}

// Generate a random private key, from the starknet library
fn generate_private_key() -> Felt {
    SigningKey::from_random().secret_scalar()
}

/// Core deployment logic that can be reused by both CLI and integration tests
pub async fn deploy_paymaster_core(params: SetupParameters, skip_user_confirmation: bool) -> Result<ServiceConfiguration, Error> {
    info!("Starting Paymaster setup for profile: {}", params.profile);

    // Load the configuration
    let chain_id = ChainID::from_string(&params.chain_id).expect("invalid chain-id");
    let default_rpc_url = Endpoint::default_rpc_url(&chain_id);
    let rpc_url = params.rpc_url.unwrap_or_else(|| default_rpc_url.to_string());
    let gas_tank_fund_in_fri = parse_units(params.fund, 18);
    let estimate_account_fund_in_fri = parse_units(params.estimate_account_fund, 18);
    let num_relayers = params.num_relayers;

    // By default, we support USDC as gas token
    let supported_tokens = HashSet::from([Token::usdc(&chain_id).address]);

    // Compute the total funding amount
    let gas_tank_reserve_in_fri = parse_units(1.0, 18);
    let estimate_account_funding_amount = estimate_account_fund_in_fri;
    let total_funding_amount = estimate_account_funding_amount + gas_tank_reserve_in_fri + gas_tank_fund_in_fri;

    // Print the parameters to the user
    info!("Using chain-id: {}", chain_id.as_identifier());
    info!("Using RPC URL: {}", rpc_url);
    info!("Nbr of relayers: {}", num_relayers);
    info!("Minimum relayer balance: {} STRK", format_units(parse_units(params.min_relayer_balance, 18), 18));
    info!(
        "Rebalancing trigger balance: {} STRK",
        format_units(parse_units(params.rebalancing_trigger_balance, 18), 18)
    );
    info!("Fund estimate account with: {} STRK", format_units(estimate_account_fund_in_fri, 18));
    info!(
        "Fund gas tank with: {} STRK(reserve) + {} STRK(fund)",
        format_units(gas_tank_reserve_in_fri, 18),
        format_units(gas_tank_fund_in_fri, 18)
    );
    info!("Total amount to fund paymaster: {} STRK", format_units(total_funding_amount, 18));
    info!("Profile path: {}", params.profile);

    // Initialize the Starknet client
    let starknet = Client::new(&Configuration {
        endpoint: rpc_url.clone(),
        chain_id,
        fallbacks: vec![],
        timeout: 10,
    });

    // Check that the initial funding is enough for rebalancing to work properly
    assert_rebalancing_configuration(
        num_relayers,
        parse_units(params.min_relayer_balance, 18),
        parse_units(params.rebalancing_trigger_balance, 18),
        gas_tank_fund_in_fri,
    )
    .await?;

    // Assert the balance of master is greater than the amount of STRK needed for the deployment (Relayers + Estimate Account)
    // If not, stop the setup execution
    assert_strk_balance(&starknet, params.master_address, total_funding_amount)
        .await
        .unwrap();

    // Ask user for confirmation before proceeding (unless skipped for tests)
    if !skip_user_confirmation {
        print!(
            "Do you want to proceed with the deployment? This will transfer {} STRK tokens to gas tank and estimate account. (y/N): ",
            format_units(total_funding_amount, 18)
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| Error::Execution(format!("Failed to read user input: {}", e)))?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            info!("Deployment cancelled by user.");
            return Err(Error::Execution("Deployment cancelled by user".to_string()));
        }
    }

    info!("Proceeding with deployment...");

    let master = starknet.initialize_account(&StarknetAccountConfiguration {
        address: params.master_address,
        private_key: params.master_pk,
    });

    // Generating private keys for accounts to be deployed
    let estimate_account_pk = generate_private_key();
    let gas_tank_pk = generate_private_key();
    let shared_relayers_pk = generate_private_key();

    /********* Build all calls needed for deployment *********/
    // Get Gas Tank deployment calls (Argent account)
    let gas_tank_tx = GasTankDeployment::build(&starknet, gas_tank_pk, gas_tank_reserve_in_fri + gas_tank_fund_in_fri).await?;

    // Get Forwarder deployment calls
    let forwarder_deployment = ForwarderDeployment::build(params.master_address, gas_tank_tx.address).await?;

    // Get Estimate Account deployment calls (represented as an account relayer -> must be whitelisted by the forwarder)
    // Always fund the estimate account with the default amount of STRK
    let estimate_account_deployment = RelayerDeployment::build_one(&starknet, forwarder_deployment.address, estimate_account_pk, estimate_account_fund_in_fri).await?;
    // We only deployed 1 estimate account (with a relayer behaviour)
    let estimate_account_address = estimate_account_deployment.address;

    // Get all relayers deployment calls
    // We don't fund the relayers with STRK, we load the gas tank instead
    let relayers_deployment = RelayerDeployment::build_many(&starknet, forwarder_deployment.address, shared_relayers_pk, num_relayers, Felt::ZERO).await?;

    // Update configuration with new values
    let configuration = ServiceConfiguration {
        verbosity: VerbosityConfiguration::from_str(&params.verbosity).unwrap(),
        starknet: StarknetConfiguration {
            endpoint: rpc_url.clone(),
            chain_id,
            fallbacks: vec![],
            timeout: params.rpc_timeout,
        },
        rpc: RPCConfiguration { port: params.rpc_port },
        prometheus: None,
        max_fee_multiplier: params.max_fee_multiplier,
        provider_fee_overhead: params.fee_overhead,
        supported_tokens,
        forwarder: forwarder_deployment.address,
        estimate_account: StarknetAccountConfiguration {
            address: estimate_account_address,
            private_key: estimate_account_pk,
        },
        gas_tank: StarknetAccountConfiguration {
            address: gas_tank_tx.address,
            private_key: gas_tank_pk,
        },
        relayers: RelayersConfiguration {
            private_key: shared_relayers_pk,
            addresses: relayers_deployment.addresses,
            min_relayer_balance: Felt::from(parse_units(params.min_relayer_balance, 18)),
            lock: DEFAULT_RELAYERS_LOCK_MODE,
            rebalancing: OptionalRebalancingConfiguration::initialize(Some(RebalancingConfiguration {
                check_interval: params.rebalancing_check_interval,
                trigger_balance: Felt::from(parse_units(params.rebalancing_trigger_balance, 18)),
                swap_config: SwapConfiguration {
                    slippage: params.swap_slippage,
                    swap_client_config: SwapClientConfigurator::AVNU(SwapClientConfiguration {
                        endpoint: Endpoint::default_swap_url(&chain_id).to_string(),
                        chain_id,
                    }),
                    max_price_impact: params.max_price_impact,
                    swap_interval: params.swap_interval,
                    min_usd_sell_amount: params.min_swap_sell_amount,
                },
            })),
        },
        price: PriceConfiguration::AVNU(AVNUPriceClientConfiguration {
            endpoint: Endpoint::default_price_url(&chain_id).to_string(),
            api_key: None,
        }),
        sponsoring: DEFAULT_SPONSORING_MODE,
    };

    // Perform rebalancing
    let rebalancing_call = perform_rebalancing(&starknet, &configuration, params.master_address, gas_tank_fund_in_fri).await?;

    // build multicall
    let mut multicall = Calls::empty();
    multicall.merge(&gas_tank_tx.calls);
    multicall.merge(&forwarder_deployment.calls);
    multicall.merge(&estimate_account_deployment.calls);
    multicall.merge(&relayers_deployment.calls);
    multicall.push(rebalancing_call);

    // run multicall
    let nonce = master.get_nonce().await.unwrap();
    let result = multicall.execute(&master, nonce).await.unwrap();

    // Wait for tx to be executed
    wait_for_transaction_success(&starknet, result.transaction_hash, params.max_check_status_attempts).await?;

    /********* Paymaster is deployed *********/
    info!(
        "✅ Paymaster contracts are successfully deployed, tx hash: {}",
        result.transaction_hash.to_fixed_hex_string()
    );

    // Write the profile to the file
    let _ = configuration.write_to_file(&params.profile);
    info!("📝 Configuration file is updated, see {}", params.profile);

    Ok(configuration)
}

// Perform initial rebalancing to distribute funds to relayers
// Initial gas tank fund is the amount of STRK to be distributed to relayers - We need to pass it to the function because of multicall (balance isn't updated inside the multicall)
async fn perform_rebalancing(starknet: &Client, configuration: &ServiceConfiguration, master_address: Felt, initial_gas_tank_fund: Felt) -> Result<Call, Error> {
    // Create a temporary relayer manager configuration for initial rebalancing
    let relayer_manager_config = RelayerManagerConfiguration {
        starknet: configuration.starknet.clone(),
        gas_tank: configuration.gas_tank.clone(),
        relayers: configuration.relayers.clone(),
        supported_tokens: configuration.supported_tokens.clone(),
    };

    // Create a relayer context and rebalancing service
    let relayer_context = RelayerContext::new(relayer_manager_config);
    let rebalancing_service = RelayerRebalancingService::new(relayer_context.clone()).await;

    // Perform initial rebalancing to distribute funds to relayers
    match rebalancing_service.try_rebalance(initial_gas_tank_fund).await {
        Ok(rebalancing_calls) => {
            if !rebalancing_calls.is_empty() {
                info!("➡️ Initial rebalancing prepared successfully");

                // Get gas tank account
                let gas_tank_account = starknet.initialize_account(&StarknetAccountConfiguration {
                    address: configuration.gas_tank.address,
                    private_key: configuration.gas_tank.private_key,
                });

                return Ok(rebalancing_calls.as_execute_from_outside_call(
                    master_address,
                    gas_tank_account,
                    configuration.gas_tank.private_key,
                    TimeBounds::valid_for(Duration::from_secs(3600)),
                ));
            }
        },
        Err(_) => {},
    }
    Err(Error::Execution(
        "⚠️ Initial rebalancing failed. Relayers will be activated by the background service.".to_string(),
    ))
}

/// CLI wrapper that uses the core deployment logic
pub async fn command_setup(params: SetupParameters) -> Result<(), Error> {
    deploy_paymaster_core(params, false).await?;
    Ok(())
}
