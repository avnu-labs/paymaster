use log::LevelFilter;
use simple_logger::SimpleLogger;

mod command;
pub mod constants;
pub mod core;
pub mod validation;

use clap::{Parser, Subcommand};

use crate::command::balance::{command_balances, BalancesCommandParameters};
use crate::command::empty::{command_empty_paymaster, EmptyPaymasterParameters};
use crate::command::quick_setup::{command_quick_setup, QuickSetupParameters};
use crate::command::relayer::deploy::{command_relayers_deploy, RelayersDeployCommandParameters};
use crate::command::relayer::rebalance::{command_relayers_rebalance, RelayersRebalanceCommandParameters};
use crate::command::setup::{command_setup, SetupParameters};
use crate::core::Error;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Deploy a new paymaster instance in 2 minutes with a minimal configuration")]
    QuickSetup(QuickSetupParameters),

    #[command(about = "Deploy and configure a new paymaster instance")]
    Setup(SetupParameters),

    #[command(about = "Deploy additional relayers to an existing paymaster")]
    RelayersDeploy(RelayersDeployCommandParameters),

    #[command(about = "Refund & rebalance STRK funds across relayers")]
    RelayersRebalance(RelayersRebalanceCommandParameters),

    #[command(about = "Check balances of paymaster accounts")]
    Balances(BalancesCommandParameters),

    #[command(about = "Empty paymaster funds back to master account")]
    Empty(EmptyPaymasterParameters),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let logger = SimpleLogger::new().with_level(LevelFilter::Info);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(LevelFilter::Info);

    let cli = Cli::parse();

    match cli.command {
        Commands::QuickSetup(params) => command_quick_setup(params).await?,
        Commands::Setup(params) => command_setup(params).await?,
        Commands::RelayersDeploy(params) => command_relayers_deploy(params).await?,
        Commands::RelayersRebalance(params) => command_relayers_rebalance(params).await?,
        Commands::Balances(params) => command_balances(params).await?,
        Commands::Empty(params) => command_empty_paymaster(params).await?,
    }

    Ok(())
}
