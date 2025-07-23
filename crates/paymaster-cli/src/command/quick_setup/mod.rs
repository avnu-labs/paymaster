use clap::Args;
use starknet::core::types::Felt;

use crate::command::setup::{deploy_paymaster_core, SetupParameters};
use crate::constants::{
    DEFAULT_INITIAL_ESTIMATE_ACCOUNT_FUND_AMOUNT, DEFAULT_INITIAL_GAS_TANK_FUND_AMOUNT, DEFAULT_MAX_CHECK_STATUS_ATTEMPTS, DEFAULT_MAX_FEE_MULTIPLIER,
    DEFAULT_MAX_PRICE_IMPACT, DEFAULT_MIN_RELAYER_BALANCE, DEFAULT_MIN_SWAP_SELL_AMOUNT, DEFAULT_PROVIDER_FEE_OVERHEAD, DEFAULT_REBALANCING_CHECK_INTERVAL,
    DEFAULT_RELAYERS_NUM, DEFAULT_RELAYERS_REBALANCE_TRIGGER_AMOUNT, DEFAULT_RPC_PORT, DEFAULT_STARKNET_TIMEOUT, DEFAULT_SWAP_INTERVAL, DEFAULT_SWAP_SLIPPAGE,
    DEFAULT_VERBOSITY,
};
use crate::core::Error;

#[derive(Args, Clone)]
pub struct QuickSetupParameters {
    #[clap(long)]
    pub rpc_url: Option<String>,

    #[clap(long)]
    pub chain_id: String,

    #[clap(long)]
    pub master_address: Felt,

    #[clap(long)]
    pub master_pk: Felt,

    #[clap(long, default_value_t = DEFAULT_INITIAL_GAS_TANK_FUND_AMOUNT)]
    pub fund: f64,

    #[clap(long, default_value = "default.json")]
    pub profile: String,
}

/// CLI wrapper that uses the core deployment logic from the setup command
pub async fn command_quick_setup(params: QuickSetupParameters) -> Result<(), Error> {
    let setup_params = SetupParameters {
        rpc_url: params.rpc_url,
        rpc_timeout: DEFAULT_STARKNET_TIMEOUT,
        rpc_port: DEFAULT_RPC_PORT,
        chain_id: params.chain_id,
        master_address: params.master_address,
        master_pk: params.master_pk,
        num_relayers: DEFAULT_RELAYERS_NUM,
        fund: params.fund,
        estimate_account_fund: DEFAULT_INITIAL_ESTIMATE_ACCOUNT_FUND_AMOUNT,
        profile: params.profile,
        max_check_status_attempts: DEFAULT_MAX_CHECK_STATUS_ATTEMPTS,
        min_swap_sell_amount: DEFAULT_MIN_SWAP_SELL_AMOUNT,
        max_fee_multiplier: DEFAULT_MAX_FEE_MULTIPLIER,
        fee_overhead: DEFAULT_PROVIDER_FEE_OVERHEAD,
        min_relayer_balance: DEFAULT_MIN_RELAYER_BALANCE,
        rebalancing_check_interval: DEFAULT_REBALANCING_CHECK_INTERVAL,
        rebalancing_trigger_balance: DEFAULT_RELAYERS_REBALANCE_TRIGGER_AMOUNT,
        swap_slippage: DEFAULT_SWAP_SLIPPAGE,
        swap_interval: DEFAULT_SWAP_INTERVAL,
        max_price_impact: DEFAULT_MAX_PRICE_IMPACT,
        verbosity: DEFAULT_VERBOSITY.to_string(),
    };
    deploy_paymaster_core(setup_params, false).await?;
    Ok(())
}
