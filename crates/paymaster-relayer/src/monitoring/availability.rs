use std::time::Duration;

use async_trait::async_trait;
use paymaster_common::metric;
use paymaster_common::service::{Error, Service};
use tokio::time;
use tracing::{error, info};

use crate::context::Context;

pub struct EnabledRelayersService {
    context: Context,
}

#[async_trait]
impl Service for EnabledRelayersService {
    type Context = Context;

    const NAME: &'static str = "EnabledRelayersService";

    async fn new(context: Self::Context) -> Self {
        Self { context }
    }

    async fn run(self) -> Result<(), Error> {
        let mut ticker = time::interval(Duration::from_secs(5));
        let mut previous_available_relayers_count = 0;
        // We want to wait a bit so the balances of the relayers are fetched
        ticker.tick().await;

        loop {
            ticker.tick().await;

            let enabled_relayers = self.context.relayers_locks.count_enabled_relayers().await;
            if previous_available_relayers_count != enabled_relayers {
                if enabled_relayers > 0 {
                    info!("{} enabled relayers", enabled_relayers);
                }
                previous_available_relayers_count = enabled_relayers;
            }
            if enabled_relayers == 0 {
                error!("No enabled relayer. Please check the STRK balance of the relayers.");
            }
            metric!(gauge[available_relayers] = enabled_relayers)
        }
    }
}
