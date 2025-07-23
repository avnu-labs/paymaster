use std::collections::HashMap;
use std::ops::Deref;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_common::cache::Expirable;
use paymaster_common::service::messaging::MessageReceiver;
use paymaster_common::service::{Error, Service};
use paymaster_common::service_check;
use starknet::core::types::{Felt, TransactionStatus};
use tokio::time;

use crate::context::Context;
use crate::lock::RelayerLock;
use crate::{Message, Relayer};

#[derive(Default)]
pub struct RelayerTransactions(HashMap<Felt, Expirable<Felt>>);

impl RelayerTransactions {
    pub fn record(&mut self, relayer: Felt, transaction_hash: Felt) {
        let entry = self.0.entry(relayer).or_insert(Expirable::empty(Duration::from_secs(20)));

        if entry.is_stale() {
            entry.refresh_with(transaction_hash)
        }
    }
}

impl Deref for RelayerTransactions {
    type Target = HashMap<Felt, Expirable<Felt>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct RelayerTransactionMonitoring {
    context: Context,
    messages: MessageReceiver<Message>,
}

#[async_trait]
impl Service for RelayerTransactionMonitoring {
    type Context = Context;

    const NAME: &'static str = "TransactionMonitor";

    async fn new(mut context: Self::Context) -> Self {
        Self {
            messages: context.messages.receiver::<Self>().subscribe_to::<Relayer>().build().await,

            context,
        }
    }

    async fn run(mut self) -> Result<(), Error> {
        let mut relayer_transactions = RelayerTransactions::default();

        let mut ticker = time::interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;

            let messages = self.messages.receive_all().await;
            for message in messages {
                let Message::Transaction { relayer, transaction_hash } = message;
                relayer_transactions.record(relayer, transaction_hash)
            }

            let mut new_relayer_transactions = RelayerTransactions::default();
            for (relayer, transaction) in relayer_transactions.iter() {
                if let Some(transaction) = self.check_transaction(*relayer, transaction).await {
                    new_relayer_transactions.record(*relayer, *transaction);
                }
            }

            relayer_transactions = new_relayer_transactions
        }
    }
}

impl RelayerTransactionMonitoring {
    async fn check_transaction(&self, relayer: Felt, transaction: &Expirable<Felt>) -> Option<Expirable<Felt>> {
        let status = service_check!(
            self.context.starknet.get_transaction_status(*transaction.deref()).await
            => return Some(transaction.clone())
        );

        match status {
            TransactionStatus::Rejected { reason: _reason } => {
                let kill_lock = RelayerLock::new(relayer, None, Duration::from_secs(0));

                service_check!(self.context.relayers_locks.release_relayer_delayed(kill_lock, 20).await => {});
                None
            },
            TransactionStatus::AcceptedOnL2(_) | TransactionStatus::AcceptedOnL1(_) => None,
            TransactionStatus::Received => Some(transaction.clone()),
        }
    }
}
