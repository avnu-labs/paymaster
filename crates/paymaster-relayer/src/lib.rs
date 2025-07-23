use std::sync::Arc;
use std::time::Instant;

use paymaster_common::service::TokioServiceManager;
use starknet::accounts::Account;
use starknet::core::types::Felt;
use thiserror::Error;
use tracing::debug;

pub use crate::context::Context;
use crate::lock::RelayerLock;

pub mod lock;

mod relayer;
pub mod swap;
pub use relayer::{LockedRelayer, Relayer, RelayerConfiguration};

mod context;
pub use context::configuration::RelayersConfiguration;
use paymaster_common::service::tracing::instrument;
pub use rebalancing::RelayerManagerConfiguration;

use crate::monitoring::availability::EnabledRelayersService;
use crate::monitoring::balance::RelayerBalanceMonitoring;
use crate::monitoring::transaction::RelayerTransactionMonitoring;

mod monitoring;
pub mod rebalancing;
pub use rebalancing::RelayerRebalancingService;

macro_rules! log_if_error {
    ($e: expr) => {
        match $e {
            Ok(v) => Ok(v),
            Err(error) => {
               paymaster_common::service::tracing::error!(message=%error);
               Err(error)
            }
        }
    };
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Lock(#[from] lock::Error),

    #[error("invalid nonce")]
    InvalidNonce,

    #[error("could not acquire relayer")]
    InvalidRelayer,

    #[error("No enabled relayer")]
    NoEnabledRelayer,

    #[error("Relayer's lock has expired")]
    RelayerLockExpired,

    #[error("execution {0}")]
    Execution(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    Transaction { relayer: Felt, transaction_hash: Felt },
}

#[derive(Clone)]
pub struct RelayerManager {
    context: Context,

    #[allow(dead_code)]
    services: Arc<TokioServiceManager<Context>>,
}

impl RelayerManager {
    pub fn new(configuration: &RelayerManagerConfiguration) -> Self {
        let context = Context::new(configuration.clone());

        let mut services = TokioServiceManager::new(context.clone());
        services.spawn::<RelayerBalanceMonitoring>();
        services.spawn::<RelayerTransactionMonitoring>();
        services.spawn::<EnabledRelayersService>();

        // Start the rebalancing service if configured
        if configuration.relayers.rebalancing.has_configuration() {
            services.spawn::<RelayerRebalancingService>();
        }

        Self {
            context,
            services: Arc::new(services),
        }
    }

    #[instrument(name = "lock_relayer", skip(self))]
    pub async fn lock_relayer(&self) -> Result<LockedRelayer, Error> {
        self.check_enabled_relayers().await?;

        let lock = log_if_error!(self.try_lock_relayer().await)?;
        let relayer = log_if_error!(self.context.relayers.acquire_relayer(&lock.address))?;
        debug!(target: "Relayers", "lock relayer {}", relayer.address().to_fixed_hex_string());

        Ok(relayer.lock(lock))
    }

    async fn try_lock_relayer(&self) -> Result<RelayerLock, Error> {
        let now = Instant::now();
        let timeout = self.context.configuration.relayers.lock.retry_timeout();

        loop {
            match self.context.relayers_locks.lock_relayer().await {
                Ok(lock) => return Ok(lock),
                Err(e) if now.elapsed() > timeout => return Err(e.into()),
                _ => continue,
            }
        }
    }

    #[instrument(name = "lock_relayer", skip(self, relayer), fields(relayer = %relayer.address().to_hex_string()))]
    pub async fn release_relayer(&self, relayer: LockedRelayer) -> Result<(), Error> {
        let (relayer, lock) = relayer.unlock();
        debug!(target: "Relayers", "release relayer {}", relayer.address().to_fixed_hex_string());

        log_if_error!(self.context.relayers_locks.release_relayer(lock).await)?;

        Ok(())
    }

    #[instrument(name = "release_relayer_delayed", skip(self, relayer), fields(relayer = %relayer.address().to_hex_string()))]
    pub async fn release_relayer_delayed(&self, relayer: LockedRelayer, delay: u64) -> Result<(), Error> {
        let (_, lock) = relayer.unlock();
        log_if_error!(self.context.relayers_locks.release_relayer_delayed(lock, delay).await)?;

        Ok(())
    }

    async fn check_enabled_relayers(&self) -> Result<(), Error> {
        if self.context.relayers_locks.count_enabled_relayers().await > 0 {
            return Ok(());
        }

        Err(Error::NoEnabledRelayer)
    }

    pub async fn count_enabled_relayers(&self) -> usize {
        self.context.relayers_locks.count_enabled_relayers().await
    }

    pub fn get_context(&self) -> &Context {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod standard_behaviors {
        use std::collections::HashSet;
        use std::time::Duration;

        use async_trait::async_trait;
        use paymaster_starknet::constants::{Endpoint, Token};
        use paymaster_starknet::{ChainID, Configuration as StarknetConfiguration, StarknetAccountConfiguration};
        use starknet::core::types::Felt;
        use starknet::macros::felt;

        use crate::lock::mock::MockLockLayer;
        use crate::lock::{LockLayerConfiguration, RelayerLock};
        use crate::rebalancing::{OptionalRebalancingConfiguration, RelayerManagerConfiguration};
        use crate::{RelayerManager, RelayersConfiguration};

        #[derive(Debug)]
        pub struct Lock;

        #[async_trait]
        impl MockLockLayer for Lock {
            fn new() -> Self
            where
                Self: Sized,
            {
                Self
            }

            async fn count_enabled_relayers(&self) -> usize {
                1
            }

            async fn lock_relayer(&self) -> Result<RelayerLock, crate::lock::Error> {
                Ok(RelayerLock::new(felt!("0x0"), None, Duration::from_secs(180)))
            }

            async fn release_relayer(&self, _lock: RelayerLock) -> Result<(), crate::lock::Error> {
                Ok(())
            }
        }

        fn configuration() -> RelayerManagerConfiguration {
            RelayerManagerConfiguration {
                starknet: StarknetConfiguration {
                    chain_id: ChainID::Sepolia,
                    endpoint: Endpoint::default_rpc_url(&ChainID::Sepolia).to_string(),
                    fallbacks: vec![],
                    timeout: 10,
                },
                supported_tokens: HashSet::from([Token::usdc(&ChainID::Sepolia).address]),
                gas_tank: StarknetAccountConfiguration {
                    address: felt!("0x0"),
                    private_key: felt!("0x0"),
                },
                relayers: RelayersConfiguration {
                    min_relayer_balance: Felt::ZERO,
                    private_key: felt!("0x0"),
                    addresses: vec![felt!("0x0")],
                    lock: LockLayerConfiguration::mock_with_timeout::<Lock>(Duration::from_secs(5)),
                    rebalancing: OptionalRebalancingConfiguration::initialize(None),
                },
            }
        }

        #[tokio::test]
        async fn acquire_release_works_properly() {
            let relayers = RelayerManager::new(&configuration());

            // Acquire relayer
            let relayer = relayers.lock_relayer().await.unwrap();

            // Release relayer
            relayers.release_relayer(relayer).await.unwrap();

            // Acquire again same relayer should work
            let _ = relayers.lock_relayer().await.unwrap();
        }
    }

    /*mod services {
        use std::collections::HashSet;
        use std::sync::Arc;
        use std::time::Duration;
        use async_trait::async_trait;
        use starknet::core::types::Felt;
        use starknet::macros::felt;
        use tokio::sync::RwLock;
        use tokio::time;
        use paymaster_starknet::Configuration as StarknetConfiguration;
        use crate::context::{Configuration};
        use crate::coordination::{CoordinationLayerParameters, Error, RelayerLock};
        use crate::coordination::mock::MockCoordinationLayer;
        use crate::{RelayerConfiguration, RelayerManager};

        #[derive(Default)]
        pub struct MockCoordination(Arc<RwLock<HashSet<Felt>>>);

        #[async_trait]
        impl MockCoordinationLayer for MockCoordination {
            async fn available_relayers(&self) -> bool {
                self.0.read().await.is_empty()
            }

            async fn enable_relayers(&self, relayers: &HashSet<Felt>) {
                self.0.write().await.extend(relayers)
            }
        }

        fn configuration() -> Configuration {
            Configuration {
                starknet: StarknetConfiguration {
                    endpoint: Endpoint::default_rpc_url(&ChainID::Sepolia),
                },
                relayers: vec![
                    RelayerConfiguration {
                        chain_id: felt!("0x0"),
                        address: felt!("0x0"),
                        private_key: felt!("0x0"),
                    }
                ],
                coordination_layer: CoordinationLayerParameters::Mock(Arc::new(MockCoordination::default()))
            }
        }

        #[tokio::test]
        async fn service_should_disable_relayer() {
            let relayers = RelayerManager::new(configuration());

            while let Ok(_) = relayers.check_available_relayers().await {
                time::sleep(Duration::from_secs(1)).await
            }
        }
    }*/
}
