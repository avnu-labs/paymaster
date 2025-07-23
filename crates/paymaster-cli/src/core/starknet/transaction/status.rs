use paymaster_starknet::Client;
use starknet::core::types::{Felt, TransactionStatus};
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::core::Error;

pub async fn wait_for_transaction_success(starknet: &Client, tx_hash: Felt, max_attempts: usize) -> Result<(), Error> {
    for _ in (0..max_attempts).rev() {
        match starknet.get_transaction_status(tx_hash).await {
            Ok(TransactionStatus::AcceptedOnL2(_) | TransactionStatus::AcceptedOnL1(_)) => {
                info!("Transaction succeeded: {}", tx_hash.to_fixed_hex_string());
                return Ok(());
            },
            Ok(TransactionStatus::Rejected { reason: _reason }) => {
                return Err(Error::Execution(format!("Transaction was rejected: {}", tx_hash.to_fixed_hex_string())));
            },
            // Do nothing, we will retry
            Ok(TransactionStatus::Received) => {},
            Err(_) => {},
        }
        // If we can't get rejected or accepted status, wait and retry (might be temporary network issue)
        sleep(Duration::from_secs(4)).await;
    }

    // If we get here, we've exhausted all attempts
    Err(Error::Execution(format!(
        "Could not confirm transaction after {} attempts: {}",
        max_attempts,
        tx_hash.to_fixed_hex_string()
    )))
}
