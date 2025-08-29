use std::ops::Deref;
use std::time::Duration;

use paymaster_common::cache::ExpirableCache;
use paymaster_common::{declare_message_identity, metric};
use paymaster_starknet::transaction::EstimatedCalls;
use paymaster_starknet::{Client, StarknetAccount, StarknetAccountConfiguration};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt, InvokeTransactionResult};
use tracing::warn;

use crate::lock::RelayerLock;
use crate::Error;

#[derive(Debug, Clone, Copy)]
pub struct RelayerConfiguration {
    pub account: StarknetAccountConfiguration,
}

#[derive(Clone)]
pub struct RelayerContext {
    pub balances: ExpirableCache<Felt, Felt>,
}

#[derive(Clone)]
pub struct Relayer {
    account: StarknetAccount,
    context: RelayerContext,
}

declare_message_identity!(Relayer);

impl Deref for Relayer {
    type Target = StarknetAccount;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl Relayer {
    pub fn new(starknet: &Client, balances: ExpirableCache<Felt, Felt>, configuration: &RelayerConfiguration) -> Self {
        let mut account = starknet.initialize_account(&configuration.account);
        account.set_block_id(BlockId::Tag(BlockTag::PreConfirmed));

        Self {
            account,
            context: RelayerContext { balances },
        }
    }

    // TODO: the semantic is not clear
    pub fn lock(self, lock: RelayerLock) -> LockedRelayer {
        LockedRelayer { lock, relayer: self }
    }

    pub async fn update_relayer_balance(&self, gas_used: Felt) {
        // Update the balance in the cache by subtracting the gas used
        self.decrement_relayer_balance(self.address(), gas_used).await;

        tracing::debug!(
            "Updated cached balance for relayer {} after transaction (gas used: {})",
            self.address().to_fixed_hex_string(),
            gas_used.to_fixed_hex_string()
        );
    }

    // Decrease valid cache balance with specified amount
    async fn decrement_relayer_balance(&self, relayer: Felt, amount: Felt) {
        let balance = self.context.balances.get_if_not_stale(&relayer);
        if let Some(prev_balance) = balance {
            let new_balance = if prev_balance > amount { prev_balance - amount } else { Felt::ZERO };
            self.context.balances.insert(relayer, new_balance, Duration::from_secs(60));
        }
        // If cached balance has already expired, the update is not required
    }
}

pub struct LockedRelayer {
    lock: RelayerLock,
    relayer: Relayer,
}

impl LockedRelayer {
    pub fn address(&self) -> Felt {
        self.relayer.address()
    }

    pub fn unlock(self) -> (Relayer, RelayerLock) {
        (self.relayer, self.lock)
    }

    pub async fn execute(&mut self, calls: &EstimatedCalls) -> Result<InvokeTransactionResult, Error> {
        metric!(counter[relayer_request] = 1, method = "execute");

        if self.lock.is_expired() {
            metric!(counter[relayer_request_error] = 1, method = "execute", error = "is_expired");

            return Err(Error::RelayerLockExpired);
        }

        let nonce = self.get_nonce().await?;
        let result = calls.execute(&self.relayer.account, nonce).await;

        match result {
            Ok(value) => {
                self.lock.nonce = Some(nonce + Felt::ONE);
                self.relayer
                    .update_relayer_balance(Felt::from(calls.estimate().overall_fee))
                    .await;
                Ok(value)
            },
            Err(paymaster_starknet::Error::InvalidNonce(_value)) => {
                metric!(counter[relayer_request_error] = 1, method = "execute", error = "invalid_nonce");

                self.invalidate_nonce();
                Err(Error::InvalidNonce)
            },
            Err(paymaster_starknet::Error::ValidationFailure(error)) if error.contains("Invalid transaction nonce of contract at address") => {
                warn!("Invalid nonce error: {}", error);
                metric!(counter[relayer_request_error] = 1, method = "execute", error = "invalid_nonce");

                self.invalidate_nonce();
                Err(Error::InvalidNonce)
            },
            Err(e) => {
                metric!(counter[relayer_request_error] = 1, method = "execute", error = e.to_string());

                Err(Error::Execution(e.to_string()))
            },
        }
    }

    fn invalidate_nonce(&mut self) {
        self.lock.nonce = None;
    }

    async fn get_nonce(&mut self) -> Result<Felt, Error> {
        if let Some(nonce) = self.lock.nonce {
            return Ok(nonce);
        }

        let nonce = self
            .relayer
            .account
            .get_nonce()
            .await
            .map_err(|e| Error::Execution(e.to_string()))?;

        self.lock.nonce = Some(nonce);
        Ok(nonce)
    }
}
