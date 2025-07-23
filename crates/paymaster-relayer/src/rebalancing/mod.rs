use std::collections::HashSet;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::service::{Error as ServiceError, Service};
use paymaster_common::task;
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::format_units;
use paymaster_starknet::transaction::{Calls, TokenTransfer};
use paymaster_starknet::{Configuration as StarknetConfiguration, StarknetAccount, StarknetAccountConfiguration};
use serde::{Deserialize, Serialize};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use tokio::time::interval;
use tracing::{error, info};

use crate::context::Context;
use crate::swap::{SwapClient, SwapConfiguration};
use crate::RelayersConfiguration;

pub struct RelayerBalance {
    relayer: Felt,
    balance: Felt,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptionalRebalancingConfiguration(Option<RebalancingConfiguration>);

impl OptionalRebalancingConfiguration {
    pub fn has_configuration(&self) -> bool {
        self.0.is_some()
    }

    pub fn trigger_balance(&self) -> Felt {
        match &self.0 {
            Some(config) => config.trigger_balance,
            _ => unimplemented!(),
        }
    }

    pub fn check_interval(&self) -> u64 {
        match &self.0 {
            Some(config) => config.check_interval,
            _ => unimplemented!(),
        }
    }

    pub fn swap_config(&self) -> &SwapConfiguration {
        match &self.0 {
            Some(config) => &config.swap_config,
            _ => unimplemented!(),
        }
    }

    pub fn initialize(config: Option<RebalancingConfiguration>) -> Self {
        OptionalRebalancingConfiguration(config)
    }

    pub fn validate(&self, min_relayer_balance: Felt) -> Result<(), ServiceError> {
        if let Some(rebalancing_config) = &self.0 {
            rebalancing_config.validate()?;

            // Ensure trigger_balance > min_relayer_balance
            if rebalancing_config.trigger_balance <= min_relayer_balance {
                return Err(ServiceError::new(
                    "trigger_balance must be greater than min_relayer_balance to ensure relayers are rebalanced before being disabled",
                ));
            }
        }
        Ok(())
    }
}

/// Configuration for the relayer rebalancing service (serializable version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RebalancingConfiguration {
    // Minimum balance to trigger refunding
    pub trigger_balance: Felt,

    // How often to check relayer balances (in seconds)
    pub check_interval: u64,

    // Configuration for the swap service
    pub swap_config: SwapConfiguration,
}

impl RebalancingConfiguration {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.check_interval <= self.swap_config.swap_interval {
            return Err(ServiceError::new(
                "check_interval must be greater than swap_interval to reduce price impact over time",
            ));
        }
        self.swap_config.validate()
    }
}

#[derive(Debug, Clone)]
pub struct RelayerManagerConfiguration {
    pub starknet: StarknetConfiguration,
    pub gas_tank: StarknetAccountConfiguration,
    pub relayers: RelayersConfiguration,
    pub supported_tokens: HashSet<Felt>,
}

impl RelayerManagerConfiguration {
    pub fn validate(&self) -> Result<(), ServiceError> {
        // Validate relayers configuration (which includes rebalancing validation)
        self.relayers.validate()?;
        Ok(())
    }
}

pub struct RelayerRebalancingService {
    context: Context,
    rebalancing_configuration: RebalancingConfiguration,
    swap_configuration: SwapConfiguration,
    gas_tank: StarknetAccount,
    supported_tokens: HashSet<Felt>,
    swap_client: SwapClient,
}

#[async_trait]
impl Service for RelayerRebalancingService {
    type Context = Context;

    const NAME: &'static str = "RelayerRebalancingService";

    async fn new(context: Context) -> Self {
        let OptionalRebalancingConfiguration(Some(rebalancing_configuration)) = context.configuration.relayers.rebalancing.clone() else {
            panic!("no rebalancing configuration")
        };
        let swap_configuration = rebalancing_configuration.swap_config.clone();
        let supported_tokens = context.configuration.supported_tokens.clone();
        let swap_client = SwapClient::new(&swap_configuration.swap_client_config);
        let gas_tank = context.starknet.initialize_account(&context.configuration.gas_tank);
        Self {
            context,
            rebalancing_configuration,
            swap_configuration,
            gas_tank,
            supported_tokens,
            swap_client,
        }
    }

    async fn run(self) -> Result<(), ServiceError> {
        // Swap interval is ensured to be < check interval
        let mut swap_check_ticker = interval(Duration::from_secs(self.swap_configuration.swap_interval));
        let check_interval = Duration::from_secs(self.rebalancing_configuration.check_interval);
        // Initialize to a time in the past to trigger rebalance on first iteration
        let mut last_check_for_rebalance_time = Instant::now() - check_interval;

        loop {
            swap_check_ticker.tick().await;
            info!("Swap interval reached, try to swap tokens to STRK");
            // Swap tokens to STRK with error handling
            let (swap_calls, swap_resulted_strk_balance) = match self.swap_to_strk_calls().await {
                Ok(result) => result,
                Err(e) => {
                    error!("Failed to batch swap tokens to STRK: {}", e);
                    // Continue with empty calls and zero balance instead of crashing
                    (Calls::new(vec![]), paymaster_starknet::math::parse_units(0.0, 18))
                },
            };

            // Check if it's time for a rebalance
            let should_try_rebalance = last_check_for_rebalance_time.elapsed() >= check_interval;

            // Create the multicall
            // It will contain the swap calls and the refill calls if any
            // it means the swap will be performed first at every iteration(swap_interval), then the rebalance if needed(check_interval)
            let mut calls = Calls::new(vec![]);
            calls.merge(&swap_calls);

            // Try to rebalance if it's time
            if should_try_rebalance {
                info!("Check interval reached, try to rebalance");
                last_check_for_rebalance_time = Instant::now();

                match self.try_rebalance(swap_resulted_strk_balance).await {
                    Ok(refill_relayers_calls) => {
                        // Add the refill calls to the multicall(may be empty)
                        calls.merge(&refill_relayers_calls);
                    },
                    Err(e) => {
                        error!("Failed to batch refill relayers: {}", e);
                    },
                }
            }

            // If there are no calls to execute, skip
            if calls.is_empty() {
                info!("Nothing to execute, skipping");
            } else {
                // Handle estimation errors gracefully
                let calls_estimate = match calls.estimate(&self.gas_tank).await {
                    Ok(estimate) => estimate,
                    Err(e) => {
                        error!("Failed to estimate calls for rebalancing, skip this round: {}", e);
                        continue; // Skip this iteration and try again next time
                    },
                };

                let nonce = match self.gas_tank.get_nonce().await {
                    Ok(nonce) => nonce,
                    Err(e) => {
                        error!("Failed to get nonce for rebalancing, skip this round: {}", e);
                        continue; // Skip this iteration and try again next time
                    },
                };

                // Execute the rebalancing with error handling
                match calls_estimate.execute(&self.gas_tank, nonce).await {
                    Ok(calls_execute) => {
                        let tx_hash = calls_execute.transaction_hash;
                        info!("Rebalancing executed, tx hash: {:?}", tx_hash);
                    },
                    Err(e) => {
                        error!("Failed to execute rebalancing: {}", e);
                        // Continue running
                    },
                }
            }
        }
    }
}

impl RelayerRebalancingService {
    async fn fetch_and_sync_relayers_balances(&self) -> Result<(), ServiceError> {
        // Get relayers out of cache
        let relayers = self
            .context
            .relayers
            .get_relayers_with_stale_balances(&self.context.configuration.relayers.addresses)
            .await;
        if relayers.is_empty() {
            info!("No relayers out of cache, skipping fetch and sync");
            return Ok(());
        }
        // If there are relayers out of cache, fetch their balances
        let mut executor = ConcurrentExecutor::new(self.context.clone(), 8);
        for relayer in relayers.iter().copied() {
            executor.register(task!(|env| {
                let balance = env
                    .starknet
                    .fetch_balance(Token::strk(env.starknet.chain_id()).address, relayer)
                    .await
                    .map_err(ServiceError::from)?;

                Ok::<(Felt, Felt), ServiceError>((relayer, balance))
            }));
        }

        let results = match executor.execute().await {
            Ok(results) => results,
            Err(e) => {
                error!("Failed to fetch relayers balances: {}", e);
                // Return OK to continue service operation, but log the error
                return Ok(());
            },
        };

        let mut successful_updates = 0;
        let total_relayers = results.len();

        for result in results {
            match result {
                Ok((relayer, balance)) => {
                    // Update the cache with the fetched balance
                    self.context.relayers.set_relayer_balance(relayer, balance).await;
                    successful_updates += 1;
                },
                Err(e) => {
                    error!("Failed to fetch balance for relayer: {}", e);
                    // Continue with other relayers instead of failing
                },
            }
        }

        info!("Successfully updated {}/{} relayer balances", successful_updates, total_relayers);
        Ok(())
    }

    async fn relayers_with_synced_balances(&self) -> Vec<RelayerBalance> {
        let mut relayers: Vec<RelayerBalance> = vec![];
        for relayer in &self.context.configuration.relayers.addresses {
            let balance = self.context.relayers.get_relayer_balance(relayer).await.unwrap_or(Felt::ZERO);
            relayers.push(RelayerBalance { relayer: *relayer, balance });
        }
        relayers
    }

    async fn has_at_least_one_relayer_below_trigger_balance(&self, relayers: &Vec<RelayerBalance>) -> bool {
        relayers
            .iter()
            .any(|relayer| relayer.balance < self.rebalancing_configuration.trigger_balance)
    }

    pub async fn try_rebalance(&self, additional_strk_balance: Felt) -> Result<Calls, ServiceError> {
        // First we fetch and sync relayers balances that are out of cache
        self.fetch_and_sync_relayers_balances().await?;

        let synced_relayers = self.relayers_with_synced_balances().await;

        // Then we check if there is at least one relayer below the trigger balance
        if self.has_at_least_one_relayer_below_trigger_balance(&synced_relayers).await {
            info!("At least one relayer below trigger balance, performing rebalance");
            // If there is at least one relayer below the trigger balance, we rebalance the relayers
            return self.do_rebalance(&synced_relayers, additional_strk_balance).await;
        } else {
            info!("No relayers below trigger balance, skipping rebalance for this round");
            Ok(Calls::new(vec![]))
        }
    }

    async fn do_rebalance(&self, relayers: &Vec<RelayerBalance>, additional_strk_balance: Felt) -> Result<Calls, ServiceError> {
        // Current gas tank balance
        let gas_tank_strk_balance = match self
            .context
            .starknet
            .fetch_balance(Token::strk(self.context.starknet.chain_id()).address, self.gas_tank.address())
            .await
        {
            Ok(balance) => balance,
            Err(e) => {
                error!("Failed to fetch gas tank balance: {}", e);
                return Err(ServiceError::from(e));
            },
        };

        // Reserve 1 STRK for gas fees - gas tank must always keep minimum balance for future transactions
        let gas_reserve = paymaster_starknet::math::parse_units(1.0, 18);
        let total_amount_available = if gas_tank_strk_balance > gas_reserve {
            (gas_tank_strk_balance - gas_reserve) + additional_strk_balance
        } else {
            additional_strk_balance
        };

        let (refill_relayers_calls, min_amount_needed) = self.refill_relayers_calls(total_amount_available, relayers).await;

        if min_amount_needed > total_amount_available {
            return Err(ServiceError::new(&format!(
                "Not enough STRK balance to refill all relayers to the min trigger balance, skipping rebalance. (missing: {} STRK)",
                format_units(min_amount_needed - total_amount_available, 18)
            )));
        }

        Ok(refill_relayers_calls)
    }

    pub async fn swap_to_strk_calls(&self) -> Result<(Calls, Felt), ServiceError> {
        // Create a call to swap each supported token to STRK
        let mut calls = Calls::new(vec![]);
        let mut accumulated_gas_swap_result = Felt::ZERO;
        let mut successful_swaps = 0;
        let total_tokens = self.supported_tokens.len();

        // Remove the STRK token from the supported tokens before swapping
        let mut supported_tokens_without_strk = self.supported_tokens.clone();
        supported_tokens_without_strk.remove(&Token::strk(self.context.starknet.chain_id()).address);

        for token in &supported_tokens_without_strk {
            // Get token balance with error handling
            let token_balance = match self.context.starknet.fetch_balance(*token, self.gas_tank.address()).await {
                Ok(balance) => balance,
                Err(e) => {
                    error!("Failed to fetch balance for token {:?}: {}", token, e);
                    continue; // Skip this token and continue with others
                },
            };

            if token_balance == Felt::ZERO {
                info!("Nothing to swap for token {:?}, omit it", token);
                continue;
            }

            // Swap token to STRK
            let (swap_calls, min_received) = match self
                .swap_client
                .swap(
                    *token,
                    Token::strk(self.context.starknet.chain_id()).address,
                    token_balance,
                    self.gas_tank.address(),
                    self.swap_configuration.slippage,
                    self.swap_configuration.max_price_impact,
                    self.swap_configuration.min_usd_sell_amount,
                )
                .await
            {
                Ok(inner_swap_calls) => inner_swap_calls,
                Err(e) => {
                    error!("Failed to swap token {:?}, omit it: {}", token, e);
                    continue;
                },
            };

            // Try to swap the token to STRK
            // If the swap fails, we skip the token
            // If the swap succeeds, we add the calls to the multicall
            // If the swap succeeds, we add the min received to the accumulated gas swap result
            let calls_to_validate = Calls::new(swap_calls);
            match calls_to_validate.estimate(&self.gas_tank).await {
                Ok(_calls_estimate) => {
                    calls.merge(&calls_to_validate);
                    accumulated_gas_swap_result += min_received;
                    successful_swaps += 1;
                },
                Err(e) => {
                    error!("Failed to estimate swap calls for token {:?}: {}, omit it", token, e);
                    continue;
                },
            };
        }

        info!("Successfully prepared {}/{} token swaps", successful_swaps, total_tokens);
        Ok((calls, accumulated_gas_swap_result))
    }

    /// Calculate the calls to refill the relayers to the target balance
    /// Consists of a multicall of transfers to the relayers
    async fn refill_relayers_calls(&self, strk_to_refill: Felt, relayers: &Vec<RelayerBalance>) -> (Calls, Felt) {
        // Calculate the target balance
        let final_target_balance = self.calculate_optimal_target_balance(strk_to_refill, relayers);

        let mut calls = Calls::new(vec![]);
        let mut min_amount_needed = Felt::ZERO;
        // Distribute the funds equally among all relayers
        for relayer in relayers {
            // Calculate how much this relayer needs to reach the final target balance
            let current_balance = relayer.balance;
            let amount_needed = if current_balance < final_target_balance {
                final_target_balance - current_balance
            } else {
                Felt::ZERO
            };

            // Only create a transfer call if the relayer needs funds
            if amount_needed > Felt::ZERO {
                calls.push(TokenTransfer::new(Token::strk(self.context.starknet.chain_id()).address, relayer.relayer, amount_needed).to_call());
                min_amount_needed += amount_needed;
            }
        }
        (calls, min_amount_needed)
    }

    /// Calculate the target balance for each relayer to achieve optimal homogeneous distribution after a rebalance.
    /// Strategy:
    /// 1) Ensure all relayers reach at least trigger_balance
    /// 2) Distribute remaining funds to achieve homogeneous final balances
    /// 3) Relayers with lower current balances get more funds to level the playing field
    /// 4) Use ALL available funds (gas tank will be emptied except for 1 STRK reserve)
    fn calculate_optimal_target_balance(&self, available_funds: Felt, relayers: &Vec<RelayerBalance>) -> Felt {
        let trigger_balance = self.rebalancing_configuration.trigger_balance;

        // If there are no relayers, return 0, there is no refill needed
        if relayers.is_empty() {
            return Felt::ZERO;
        }

        // Simple binary search approach to find the target balance that uses exactly all available funds
        // We need to find the target T such that sum(max(0, T - balance_i)) = available_funds

        // Start with minimum possible target (trigger_balance)
        let mut low = trigger_balance;
        // Maximum possible target: if we gave all funds to the relayer with lowest balance
        let min_balance = relayers.iter().map(|r| r.balance).min().unwrap_or(Felt::ZERO);
        let mut high = min_balance + available_funds;

        // Binary search to find exact target
        while low < high {
            let mid_u64 = (low.try_into().unwrap_or(0u128) + high.try_into().unwrap_or(0u128)) / 2;
            let mid = Felt::from(mid_u64);

            // Calculate total funds needed to bring all relayers to this target
            let mut funds_needed = Felt::ZERO;
            for relayer in relayers {
                if relayer.balance < mid {
                    funds_needed += mid - relayer.balance;
                }
            }

            if funds_needed == available_funds {
                return mid;
            } else if funds_needed < available_funds {
                low = mid + Felt::ONE;
            } else {
                high = mid;
            }
        }

        // If binary search doesn't find exact match, return the closest target that doesn't exceed available funds
        let mut best_target = low;
        let mut funds_needed = Felt::ZERO;
        for relayer in relayers {
            if relayer.balance < best_target {
                funds_needed += best_target - relayer.balance;
            }
        }

        // If we still need more funds than available, reduce target
        if funds_needed > available_funds {
            best_target = if best_target > Felt::ONE { best_target - Felt::ONE } else { best_target };
        }

        best_target.max(trigger_balance)
    }
}

#[cfg(test)]
mod rebalancing_tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use async_trait::async_trait;
    use paymaster_common::service::Service;
    use paymaster_starknet::constants::Token;
    use paymaster_starknet::math::parse_units;
    use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
    use paymaster_starknet::{ChainID, Configuration as StarknetConfiguration};
    use starknet::core::types::Felt;

    use crate::lock::mock::MockLockLayer;
    use crate::lock::{LockLayerConfiguration, RelayerLock};
    use crate::rebalancing::{OptionalRebalancingConfiguration, RebalancingConfiguration, RelayerBalance};
    use crate::swap::client::mock::MockSimpleSwap;
    use crate::swap::{SwapClientConfigurator, SwapConfiguration};
    use crate::{Context, RelayerManagerConfiguration, RelayerRebalancingService, RelayersConfiguration};

    #[derive(Debug)]
    pub struct MockLock;

    #[async_trait]
    impl MockLockLayer for MockLock {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn count_enabled_relayers(&self) -> usize {
            1
        }

        async fn set_enabled_relayers(&self, _relayers: &HashSet<Felt>) {}

        async fn lock_relayer(&self) -> Result<RelayerLock, crate::lock::Error> {
            Ok(RelayerLock::new(StarknetTestEnvironment::RELAYER_1, None, Duration::from_secs(30)))
        }

        async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), crate::lock::Error> {
            Ok(())
        }
    }

    fn setup_mock_configuration(
        trigger_balance: Felt,
        check_interval: u64,
        swap_interval: u64,
        max_price_impact: f64,
        slippage: f64,
        relayers: Vec<Felt>,
        min_relayer_balance: Felt,
        min_usd_sell_amount: f64,
    ) -> RelayerManagerConfiguration {
        RelayerManagerConfiguration {
            starknet: StarknetConfiguration {
                chain_id: ChainID::Sepolia,
                endpoint: "http://localhost:5050".to_string(),
                fallbacks: vec![],
                timeout: 10,
            },
            supported_tokens: HashSet::from([Token::usdc(&ChainID::Sepolia).address]),
            relayers: RelayersConfiguration {
                private_key: StarknetTestEnvironment::RELAYER_PRIVATE_KEY,
                addresses: relayers,
                min_relayer_balance,
                lock: LockLayerConfiguration::mock_with_timeout::<MockLock>(Duration::from_secs(5)),
                rebalancing: OptionalRebalancingConfiguration::initialize(Some(RebalancingConfiguration {
                    trigger_balance,
                    check_interval,
                    swap_config: SwapConfiguration {
                        swap_interval,
                        max_price_impact,
                        slippage,
                        swap_client_config: SwapClientConfigurator::mock::<MockSimpleSwap>(),
                        min_usd_sell_amount,
                    },
                })),
            },
            gas_tank: StarknetTestEnvironment::GAS_TANK,
        }
    }

    #[tokio::test]
    async fn test_rebalance_with_no_relayers_below_trigger_balance() {
        let trigger_balance = Felt::from(2000u64); // 2000 fri
        let min_relayer_balance = Felt::from(1000u64); // 1000 fri
        let check_interval = 100;
        let swap_interval = 10;
        let max_price_impact = 0.08;
        let slippage = 0.05;
        let min_usd_sell_amount = 0.01;

        let relayers = vec![
            StarknetTestEnvironment::RELAYER_1,
            StarknetTestEnvironment::RELAYER_2,
            StarknetTestEnvironment::RELAYER_3,
        ];

        let configuration = setup_mock_configuration(
            trigger_balance,
            check_interval,
            swap_interval,
            max_price_impact,
            slippage,
            relayers.clone(),
            min_relayer_balance,
            min_usd_sell_amount,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Mock high balances for all relayers (above trigger)
        for relayer in &relayers {
            service
                .context
                .relayers
                .set_relayer_balance(*relayer, Felt::from(5000u64))
                .await;
        }

        let additional_strk_balance = Felt::ZERO;
        let calls = service.try_rebalance(additional_strk_balance).await.unwrap();

        // No relayer is below trigger, so no calls should be generated
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn test_rebalance_with_relayers_below_trigger_balance() {
        let trigger_balance = Felt::from(1000u64); // 1000 fri
        let min_relayer_balance = Felt::from(500u64); // 500 fri
        let check_interval = 100;
        let swap_interval = 10;
        let max_price_impact = 0.08;
        let slippage = 0.05;
        let min_usd_sell_amount = 0.01;
        let relayers = vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2];

        let configuration = setup_mock_configuration(
            trigger_balance,
            check_interval,
            swap_interval,
            max_price_impact,
            slippage,
            relayers.clone(),
            min_relayer_balance,
            min_usd_sell_amount,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Direct test of the logic with mocked RelayerBalance
        let mock_relayers = vec![
            RelayerBalance {
                relayer: relayers[0],
                balance: Felt::from(500u64), // Below trigger
            },
            RelayerBalance {
                relayer: relayers[1],
                balance: Felt::from(800u64), // Below trigger
            },
        ];

        // Test has_at_least_one_relayer_below_trigger_balance
        let has_below = service.has_at_least_one_relayer_below_trigger_balance(&mock_relayers).await;
        assert!(has_below);

        // Test refill_relayers_calls directly (without network calls)
        let available_strk = Felt::from(10000u64);
        let (calls, min_amount_needed) = service.refill_relayers_calls(available_strk, &mock_relayers).await;

        // Calls should be generated to refill the relayers
        assert!(!calls.is_empty());
        assert_eq!(calls.len(), 2); // One call for each relayer
        assert!(min_amount_needed > Felt::ZERO);
    }

    #[tokio::test]
    async fn test_insufficient_funds_for_rebalance() {
        let trigger_balance = Felt::from(1000u64);
        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        let relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(100u64), // Very low balance
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(200u64), // Very low balance
            },
        ];

        // Test with insufficient funds
        let insufficient_strk = Felt::from(500u64); // Not enough to bring both to trigger
        let (calls, min_amount_needed) = service.refill_relayers_calls(insufficient_strk, &relayers).await;

        // Should still generate calls, but min_amount_needed should be higher than available
        assert!(!calls.is_empty());
        assert!(min_amount_needed > insufficient_strk);
    }

    #[tokio::test]
    async fn test_calculate_optimal_target_balance_1() {
        let trigger_balance = Felt::from(1000u64);
        let available_funds = Felt::from(5000u64);

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with relayers having different balances
        let relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(500u64), // Below trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(800u64), // Below trigger
            },
        ];

        let target_balance_should_be = Felt::from(3150u64); // 2350 + 800 = 3150
        let target_balance = service.calculate_optimal_target_balance(available_funds, &relayers);

        // Target should be >= trigger_balance and distributed homogeneously
        assert_eq!(target_balance, target_balance_should_be);

        // Assert all available funds are distributed homogeneously
        let funding_relayer_1 = target_balance_should_be - relayers[0].balance;
        let funding_relayer_2 = target_balance_should_be - relayers[1].balance;
        assert_eq!(funding_relayer_1 + funding_relayer_2, available_funds);
    }

    #[tokio::test]
    async fn test_calculate_optimal_target_balance_2() {
        let trigger_balance = Felt::from(1000u64);
        let available_funds = Felt::from(2000u64);

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![
                StarknetTestEnvironment::RELAYER_1,
                StarknetTestEnvironment::RELAYER_2,
                StarknetTestEnvironment::RELAYER_3,
            ],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with one relayer already having sufficient funds
        let relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(500u64), // Below trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(1500u64), // Above trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_3,
                balance: Felt::from(800u64), // Below trigger
            },
        ];

        let target_balance_should_be = Felt::from(1600u64);
        let target_balance = service.calculate_optimal_target_balance(available_funds, &relayers);

        // Target should be >= trigger_balance and distributed homogeneously
        assert_eq!(target_balance, target_balance_should_be);

        // Assert all available funds are distributed homogeneously
        let funding_relayer_1 = if relayers[0].balance < target_balance_should_be {
            target_balance_should_be - relayers[0].balance
        } else {
            Felt::ZERO
        };
        let funding_relayer_2 = if relayers[1].balance < target_balance_should_be {
            target_balance_should_be - relayers[1].balance
        } else {
            Felt::ZERO
        };
        let funding_relayer_3 = if relayers[2].balance < target_balance_should_be {
            target_balance_should_be - relayers[2].balance
        } else {
            Felt::ZERO
        };
        assert_eq!(funding_relayer_1 + funding_relayer_2 + funding_relayer_3, available_funds);
    }

    #[tokio::test]
    async fn test_calculate_optimal_target_balance_3() {
        let trigger_balance = Felt::from(parse_units(8.0, 18));
        let available_funds = Felt::from(parse_units(193.88480268654803, 18));

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![
                StarknetTestEnvironment::RELAYER_1,
                StarknetTestEnvironment::RELAYER_2,
                StarknetTestEnvironment::RELAYER_3,
            ],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with one relayer already having sufficient funds
        let relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(parse_units(8.0, 18)), // Above trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(parse_units(8.0, 18)), // Above trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_3,
                balance: Felt::from(parse_units(1.0, 18)), // Below trigger
            },
        ];

        let target_balance_should_be = Felt::from(parse_units(70.29493423, 18));
        let target_balance = service.calculate_optimal_target_balance(available_funds, &relayers);

        // Target should be >= trigger_balance and distributed homogeneously
        assert_eq!(target_balance, target_balance_should_be);

        // Assert all available funds are distributed homogeneously
        let funding_relayer_1 = if relayers[0].balance < target_balance_should_be {
            target_balance_should_be - relayers[0].balance
        } else {
            Felt::ZERO
        };
        let funding_relayer_2 = if relayers[1].balance < target_balance_should_be {
            target_balance_should_be - relayers[1].balance
        } else {
            Felt::ZERO
        };
        let funding_relayer_3 = if relayers[2].balance < target_balance_should_be {
            target_balance_should_be - relayers[2].balance
        } else {
            Felt::ZERO
        };
        assert_eq!(funding_relayer_1 + funding_relayer_2 + funding_relayer_3, available_funds);
    }

    #[tokio::test]
    async fn test_has_at_least_one_relayer_below_trigger_balance() {
        let trigger_balance = Felt::from(1000u64);

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with one relayer below trigger
        let relayers_below = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(500u64), // Below trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(1500u64), // Above trigger
            },
        ];

        let has_below = service.has_at_least_one_relayer_below_trigger_balance(&relayers_below).await;
        assert!(has_below);

        // Test with all relayers above trigger
        let relayers_above = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(1500u64), // Above trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(2000u64), // Above trigger
            },
        ];

        let has_above = service.has_at_least_one_relayer_below_trigger_balance(&relayers_above).await;
        assert!(!has_above);
    }

    #[tokio::test]
    async fn test_refill_relayers_calls() {
        let trigger_balance = Felt::from(1000u64);
        let available_strk = Felt::from(5000u64);

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        let relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(500u64), // Needs 500 to reach trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(800u64), // Needs 200 to reach trigger
            },
        ];

        let (calls, min_amount_needed) = service.refill_relayers_calls(available_strk, &relayers).await;

        // Should have calls for both relayers
        assert_eq!(calls.len(), 2);

        // Calculate expected minimum amount needed
        let target_balance = service.calculate_optimal_target_balance(available_strk, &relayers);
        let expected_min_amount = (target_balance - Felt::from(500u64)) + (target_balance - Felt::from(800u64));
        assert_eq!(min_amount_needed, expected_min_amount);
    }

    #[tokio::test]
    async fn test_empty_relayers_list() {
        let trigger_balance = Felt::from(1000u64);
        // Use a valid configuration (with at least one relayer for Context validation)
        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with empty relayers list (for business logic)
        let empty_relayers = vec![];
        let available_funds = Felt::from(5000u64);

        let target_balance = service.calculate_optimal_target_balance(available_funds, &empty_relayers);
        assert_eq!(target_balance, Felt::ZERO);

        let (calls, min_amount_needed) = service.refill_relayers_calls(available_funds, &empty_relayers).await;
        assert!(calls.is_empty());
        assert_eq!(min_amount_needed, Felt::ZERO);
    }

    #[tokio::test]
    async fn test_trigger_balance_validation() {
        // Test case where trigger_balance <= min_relayer_balance (should fail)
        let trigger_balance = Felt::from(500u64); // 500 fri
        let min_relayer_balance = Felt::from(1000u64); // 1000 fri (higher than trigger)

        let configuration = setup_mock_configuration(
            trigger_balance,
            100,                                      // check_interval
            10,                                       // swap_interval
            0.08,                                     // max_price_impact
            0.05,                                     // slippage
            vec![StarknetTestEnvironment::RELAYER_1], // relayers
            min_relayer_balance,
            0.01,
        );

        // This should fail validation
        let validation_result = configuration.validate();
        assert!(validation_result.is_err());

        let error_message = validation_result.unwrap_err().to_string();
        assert!(error_message.contains("trigger_balance must be greater than min_relayer_balance"));

        // Test case where trigger_balance > min_relayer_balance (should pass)
        let trigger_balance_valid = Felt::from(2000u64); // 2000 fri
        let min_relayer_balance_valid = Felt::from(1000u64); // 1000 fri

        let configuration_valid = setup_mock_configuration(
            trigger_balance_valid,
            100,                                      // check_interval
            10,                                       // swap_interval
            0.08,                                     // max_price_impact
            0.05,                                     // slippage
            vec![StarknetTestEnvironment::RELAYER_1], // relayers
            min_relayer_balance_valid,
            0.01,
        );

        // This should pass validation
        let validation_result_valid = configuration_valid.validate();
        assert!(validation_result_valid.is_ok());
    }

    #[tokio::test]
    async fn test_all_relayers_above_trigger() {
        let trigger_balance = Felt::from(1000u64);
        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with all relayers already above trigger
        let wealthy_relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: Felt::from(2000u64), // Already above trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: Felt::from(1500u64), // Already above trigger
            },
        ];

        let available_funds = Felt::from(5000u64);

        // No relayer is below trigger
        let has_below = service.has_at_least_one_relayer_below_trigger_balance(&wealthy_relayers).await;
        assert!(!has_below);

        // calculate_optimal_target_balance should continue iterating until
        // all eligible relayers are excluded or optimal distribution is reached
        let target_balance = service.calculate_optimal_target_balance(available_funds, &wealthy_relayers);

        // Since relayers already have more than trigger, algorithm should
        // continue calculating higher target until optimal distribution is achieved
        // In this case, they have 2000 and 1500, so final target could be higher
        assert!(target_balance >= trigger_balance);

        // If target is higher than relayer balances, calls will be generated
        let (calls, min_amount_needed) = service.refill_relayers_calls(available_funds, &wealthy_relayers).await;

        // Algorithm distributes available funds to achieve homogeneous balances
        // Even if relayers already have more than trigger, they can receive more
        if target_balance > wealthy_relayers.iter().map(|r| r.balance).max().unwrap_or(Felt::ZERO) {
            assert!(!calls.is_empty());
            assert!(min_amount_needed > Felt::ZERO);
        } else {
            assert!(calls.is_empty());
            assert_eq!(min_amount_needed, Felt::ZERO);
        }
    }

    #[tokio::test]
    async fn test_exact_trigger_balance() {
        let trigger_balance = Felt::from(1000u64);
        let configuration = setup_mock_configuration(
            trigger_balance,
            100,
            10,
            0.08,
            0.05,
            vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2],
            Felt::from(500u64),
            0.01,
        );

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        // Test with relayers exactly at trigger balance
        let exact_relayers = vec![
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_1,
                balance: trigger_balance, // Exactly at trigger
            },
            RelayerBalance {
                relayer: StarknetTestEnvironment::RELAYER_2,
                balance: trigger_balance, // Exactly at trigger
            },
        ];

        let available_funds = Felt::from(5000u64);

        // Relayers are exactly at trigger, so not below
        let has_below = service.has_at_least_one_relayer_below_trigger_balance(&exact_relayers).await;
        assert!(!has_below);

        // Calls can be generated since algorithm distributes fairly
        let (calls, _min_amount_needed) = service.refill_relayers_calls(available_funds, &exact_relayers).await;

        // Verify that calculated target is correct
        let target_balance = service.calculate_optimal_target_balance(available_funds, &exact_relayers);
        let remaining_u64: u64 = available_funds.try_into().unwrap_or(0u64);
        let additional_per_relayer = Felt::from(remaining_u64 / 2u64);
        let expected_target = trigger_balance + additional_per_relayer;
        assert_eq!(target_balance, expected_target);

        // If target is > trigger_balance, then calls will be generated
        if target_balance > trigger_balance {
            assert!(!calls.is_empty());
            assert_eq!(calls.len(), 2);
        } else {
            assert!(calls.is_empty());
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use async_trait::async_trait;
    use paymaster_common::service::Service;
    use paymaster_execution::testing::TestEnvironment;
    use paymaster_starknet::constants::Token;
    use paymaster_starknet::math::format_units;
    use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
    use paymaster_starknet::transaction::TokenTransfer;
    use starknet::accounts::{Account, ConnectedAccount};
    use starknet::core::types::{Felt, NonZeroFelt};

    use crate::lock::mock::MockLockLayer;
    use crate::lock::{LockLayerConfiguration, RelayerLock};
    use crate::rebalancing::{OptionalRebalancingConfiguration, RebalancingConfiguration};
    use crate::swap::client::mock::MockSimpleSwap;
    use crate::swap::{SwapClientConfigurator, SwapConfiguration};
    use crate::{Context, RelayerManagerConfiguration, RelayerRebalancingService, RelayersConfiguration};

    #[derive(Debug)]
    pub struct IntegrationMockLock;

    #[async_trait]
    impl MockLockLayer for IntegrationMockLock {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn count_enabled_relayers(&self) -> usize {
            3
        }

        async fn set_enabled_relayers(&self, _relayers: &HashSet<Felt>) {}

        async fn lock_relayer(&self) -> Result<RelayerLock, crate::lock::Error> {
            Ok(RelayerLock::new(StarknetTestEnvironment::RELAYER_1, None, Duration::from_secs(30)))
        }

        async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), crate::lock::Error> {
            Ok(())
        }
    }

    /// Full integration test for rebalancing with devnet
    /// This test:
    /// 1. Starts a Starknet devnet
    /// 2. Configures relayers with low balances
    /// 3. Funds STRK to the gas tank
    /// 4. Executes the complete rebalancing flow
    /// 5. Verifies that balances have been updated
    #[tokio::test]
    async fn test_full_rebalancing_flow_with_devnet() {
        // Setup test environment with devnet
        let test_env = TestEnvironment::new().await;
        let gas_tank_account = test_env.starknet.initialize_account(&StarknetTestEnvironment::GAS_TANK);

        // Rebalancing configuration
        let trigger_balance = Felt::from(1000000000000000000u128); // 1 STRK in fri (18 decimals)
        let min_relayer_balance = Felt::from(500000000000000000u128); // 0.5 STRK
        let check_interval = 60; // 60 seconds
        let swap_interval = 30; // 30 seconds

        let relayer_addresses = vec![
            StarknetTestEnvironment::RELAYER_1,
            StarknetTestEnvironment::RELAYER_2,
            StarknetTestEnvironment::RELAYER_3,
        ];

        let configuration = RelayerManagerConfiguration {
            starknet: test_env.starknet.configuration(),
            supported_tokens: HashSet::from([Token::usdc(test_env.starknet.chain_id()).address]),
            relayers: RelayersConfiguration {
                private_key: StarknetTestEnvironment::RELAYER_PRIVATE_KEY,
                addresses: relayer_addresses.clone(),
                min_relayer_balance,
                lock: LockLayerConfiguration::mock_with_timeout::<IntegrationMockLock>(Duration::from_secs(10)),
                rebalancing: OptionalRebalancingConfiguration::initialize(Some(RebalancingConfiguration {
                    trigger_balance,
                    check_interval,
                    swap_config: SwapConfiguration {
                        swap_interval,
                        max_price_impact: 0.08,
                        slippage: 0.05,
                        swap_client_config: SwapClientConfigurator::mock::<MockSimpleSwap>(),
                        min_usd_sell_amount: 0.01,
                    },
                })),
            },
            gas_tank: StarknetTestEnvironment::GAS_TANK,
        };

        // Create rebalancing service
        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        println!("🏗️  Setting up test environment...");

        // 1. Check initial relayer balances (probably 0)
        println!("📊 Checking initial relayer balances...");
        for (i, relayer_address) in relayer_addresses.iter().enumerate() {
            let initial_balance = test_env
                .starknet
                .fetch_balance(Token::strk(test_env.starknet.chain_id()).address, *relayer_address)
                .await
                .unwrap();
            println!("  Relayer {} initial balance: {} STRK", i + 1, initial_balance);
        }

        // 2. Fund the gas tank with STRK for rebalancing
        let gas_tank_funding_amount = Felt::from(10000000000000000000u128); // 10 STRK
        println!("💰 Funding gas tank with {} STRK...", format_units(gas_tank_funding_amount, 18));

        // Transfer STRK from ACCOUNT_1 to gas tank
        let funding_account = test_env.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);
        test_env
            .starknet
            .transfer_token(
                &funding_account,
                &TokenTransfer::new(Token::strk(test_env.starknet.chain_id()).address, gas_tank_account.address(), gas_tank_funding_amount),
            )
            .await;

        // Verify gas tank balance
        let gas_tank_balance_after_funding = test_env
            .starknet
            .fetch_balance(Token::strk(test_env.starknet.chain_id()).address, gas_tank_account.address())
            .await
            .unwrap();
        println!("✅ Gas tank balance: {} STRK", format_units(gas_tank_balance_after_funding, 18));
        assert!(gas_tank_balance_after_funding >= gas_tank_funding_amount);

        // 3. Simulate low balances for relayers using cache
        println!("🔧 Setting low balances for relayers in cache...");
        let low_balance_amount = Felt::from(100000000000000000u128); // 0.1 STRK
        for relayer_address in &relayer_addresses {
            service
                .context
                .relayers
                .set_relayer_balance(*relayer_address, low_balance_amount)
                .await;
        }

        // 4. Execute rebalancing
        println!("⚖️  Executing rebalancing...");
        let additional_strk_from_swap = Felt::ZERO; // No additional funds from swap

        // Test try_rebalance
        let rebalancing_calls = service.try_rebalance(additional_strk_from_swap).await.unwrap();
        let synced_relayers = service.relayers_with_synced_balances().await;
        let predicted_final_relayer_balances = service.calculate_optimal_target_balance(gas_tank_balance_after_funding - 1, &synced_relayers);

        println!("📋 Generated {} rebalancing calls", rebalancing_calls.len());
        assert!(!rebalancing_calls.is_empty(), "Rebalancing should generate calls for low-balance relayers");

        // 5. Estimate and execute rebalancing calls
        println!("🔧 Estimating rebalancing transaction...");
        let estimated_calls = rebalancing_calls.estimate(&gas_tank_account).await.unwrap();

        let gas_tank_nonce = gas_tank_account.get_nonce().await.unwrap();
        println!("📝 Executing rebalancing with nonce: {}", gas_tank_nonce);

        let execution_result = estimated_calls.execute(&gas_tank_account, gas_tank_nonce).await.unwrap();
        println!("✅ Rebalancing executed! Transaction hash: {:#x}", execution_result.transaction_hash);

        // 6. Wait for transaction to be mined
        println!("⏳ Waiting for transaction to be mined...");
        let mut mining_attempts = 0;
        while mining_attempts < 30 {
            match test_env
                .starknet
                .get_transaction_receipt(execution_result.transaction_hash)
                .await
            {
                Ok(_receipt) => {
                    println!("✅ Transaction mined successfully!");
                    break;
                },
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    mining_attempts += 1;
                },
            }
        }

        // 7. Verify new relayer balances
        println!("📊 Checking final relayer balances...");
        for (i, relayer_address) in relayer_addresses.iter().enumerate() {
            let final_balance = test_env
                .starknet
                .fetch_balance(Token::strk(test_env.starknet.chain_id()).address, *relayer_address)
                .await
                .unwrap();
            println!("  Relayer {} final balance: {} STRK", i + 1, format_units(final_balance, 18));
            println!("  Predicted final balance: {} STRK", format_units(predicted_final_relayer_balances, 18));
            // Verify that balances are equal to the predicted final balances
            // allow 1% of the predicted final balance as tolerance
            // calculate_optimal_target_balance could be off by some amount after converged, so we allow a tolerance
            let predicted_final_balance_tolerance = predicted_final_relayer_balances
                .div_rem(&NonZeroFelt::from_felt_unchecked(Felt::from(100)))
                .0;
            assert!(
                final_balance >= predicted_final_relayer_balances - predicted_final_balance_tolerance
                    && final_balance <= predicted_final_relayer_balances + predicted_final_balance_tolerance,
                "Relayer {} balance should be equal to the predicted final balance after rebalancing",
                i + 1
            );
        }

        // 8. Verify that gas tank balance decreased
        let final_gas_tank_balance = test_env
            .starknet
            .fetch_balance(Token::strk(test_env.starknet.chain_id()).address, gas_tank_account.address())
            .await
            .unwrap();
        println!("💰 Final gas tank balance: {} STRK", format_units(final_gas_tank_balance, 18));
        assert!(
            final_gas_tank_balance < gas_tank_balance_after_funding,
            "Gas tank balance should have decreased after rebalancing"
        );

        println!("🎉 Full rebalancing integration test completed successfully!");
    }

    /// Integration test to verify behavior when no rebalancing is needed
    #[tokio::test]
    async fn test_no_rebalancing_needed_with_devnet() {
        let test_env = TestEnvironment::new().await;

        let trigger_balance = Felt::from(1000000000000000000u128); // 1 STRK
        let relayer_addresses = vec![StarknetTestEnvironment::RELAYER_1, StarknetTestEnvironment::RELAYER_2];

        let configuration = RelayerManagerConfiguration {
            starknet: test_env.starknet.configuration(),
            supported_tokens: HashSet::from([Token::usdc(test_env.starknet.chain_id()).address]),
            relayers: RelayersConfiguration {
                private_key: StarknetTestEnvironment::RELAYER_PRIVATE_KEY,
                addresses: relayer_addresses.clone(),
                min_relayer_balance: Felt::from(500000000000000000u128),
                lock: LockLayerConfiguration::mock_with_timeout::<IntegrationMockLock>(Duration::from_secs(10)),
                rebalancing: OptionalRebalancingConfiguration::initialize(Some(RebalancingConfiguration {
                    trigger_balance,
                    check_interval: 60,
                    swap_config: SwapConfiguration {
                        swap_interval: 30,
                        max_price_impact: 0.08,
                        slippage: 0.05,
                        swap_client_config: SwapClientConfigurator::mock::<MockSimpleSwap>(),
                        min_usd_sell_amount: 0.01,
                    },
                })),
            },
            gas_tank: StarknetTestEnvironment::GAS_TANK,
        };

        let context = Context::new(configuration);
        let service = RelayerRebalancingService::new(context).await;

        println!("🔧 Setting high balances for relayers (above trigger)...");
        // Simulate high balances (above trigger) for all relayers
        for relayer_address in &relayer_addresses {
            service
                .context
                .relayers
                .set_relayer_balance(*relayer_address, Felt::from(5000000000000000000u128))
                .await; // 5 STRK
        }

        println!("⚖️  Testing rebalancing (should not trigger)...");
        let rebalancing_calls = service.try_rebalance(Felt::ZERO).await.unwrap();

        println!("📋 Generated {} calls (should be 0)", rebalancing_calls.len());
        assert!(rebalancing_calls.is_empty(), "No rebalancing should be needed when all relayers are above trigger");

        println!("✅ No rebalancing test completed successfully!");
    }
}
