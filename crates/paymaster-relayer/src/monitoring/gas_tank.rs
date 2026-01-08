use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::service::{Error, Service};
use paymaster_common::{metric, service_check, task};
use paymaster_starknet::constants::Token;
use paymaster_starknet::math::denormalize_felt;
use starknet::core::types::Felt;
use tokio::time;
use tracing::warn;

use crate::Context;

pub struct GasTankBalanceMonitoring {
    context: Context,
    gas_tank_address: Felt,
    supported_tokens: HashSet<Felt>,
}

#[async_trait]
impl Service for GasTankBalanceMonitoring {
    type Context = Context;

    const NAME: &'static str = "GasTankBalance";

    async fn new(context: Context) -> Self {
        Self {
            gas_tank_address: context.configuration.gas_tank.address,
            supported_tokens: context.configuration.supported_tokens.clone(),
            context,
        }
    }

    async fn run(self) -> Result<(), Error> {
        let mut ticker = time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            service_check!(self.fetch_and_publish_gas_tank_balance().await => continue);
        }
    }
}

impl GasTankBalanceMonitoring {
    async fn fetch_and_publish_gas_tank_balance(&self) -> Result<(), Error> {
        // Fetch balances for all supported tokens in parallel
        let mut executor = ConcurrentExecutor::new(self.context.clone(), 8);

        for token in self.supported_tokens.iter().cloned() {
            let gas_tank_address = self.gas_tank_address;
            executor.register(task!(|ctx| {
                ctx.starknet
                    .fetch_balance(token, gas_tank_address)
                    .await
                    .map(|balance| (token, balance))
            }));
        }

        let results = executor.execute().await.map_err(Error::from)?;

        // Convert each balance to STRK and sum
        let mut total_balance_in_strk: f64 = 0.0;

        for result in results {
            let (token, balance) = service_check!(result => continue);

            if balance == Felt::ZERO {
                continue;
            }

            // Convert to STRK (no conversion needed if already STRK)
            let balance_in_strk = if token == Token::STRK_ADDRESS {
                balance
            } else {
                match self.context.price.convert_token_to_strk(token, balance).await {
                    Ok(converted) => converted,
                    Err(e) => {
                        warn!("Failed to convert token {} balance to STRK: {}. Skipping this token.", token.to_fixed_hex_string(), e);
                        continue;
                    },
                }
            };

            total_balance_in_strk += denormalize_felt(balance_in_strk, 18);
        }

        // Publish metric
        metric!(gauge[gas_tank_balance_in_strk] = total_balance_in_strk);

        Ok(())
    }
}
