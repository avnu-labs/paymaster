use std::ops::Deref;

use serde::{Deserialize, Serialize};
use starknet::accounts::{Account, AccountError, ConnectedAccount};
use starknet::core::types::{
    BlockId, BlockTag, BroadcastedInvokeTransactionV3, BroadcastedTransaction, Call, DataAvailabilityMode, Felt, InvokeTransactionResult, ResourceBounds,
    ResourceBoundsMapping,
};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::SigningKey;
use tracing::error;

use crate::transaction::{ExecuteFromOutsideMessage, ExecuteFromOutsideParameters, PaymasterVersion, TimeBounds, TransactionGasEstimate};
use crate::{ChainID, Error, StarknetAccount};

mod calldata;
pub use calldata::{AsCalldata, CalldataBuilder};
mod transfer;
pub use transfer::{StrkTransfer, TokenTransfer};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
pub struct Calls(Vec<Call>);

impl Deref for Calls {
    type Target = Vec<Call>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsCalldata for Calls {
    fn encode(&self) -> Vec<Felt> {
        let mut calldata = vec![Felt::from(self.0.len())];
        for call in &self.0 {
            calldata.extend(call.encode())
        }

        calldata
    }
}

impl Calls {
    pub fn new(calls: Vec<Call>) -> Self {
        Self(calls)
    }

    pub fn empty() -> Self {
        Self(vec![])
    }

    pub fn with_estimate(self, estimate: TransactionGasEstimate) -> EstimatedCalls {
        EstimatedCalls { calls: self, estimate }
    }

    pub async fn estimate(&self, account: &StarknetAccount, tip: Option<u64>) -> Result<EstimatedCalls, Error> {
        let tip = match tip {
            None => {
                let block = account.provider().get_block_with_txs(BlockId::Tag(BlockTag::Latest)).await?;
                block.median_tip()
            },
            Some(tip) => tip,
        };

        let result = account.execute_v3(self.to_vec()).tip(tip).estimate_fee().await?;

        Ok(self.clone().with_estimate(TransactionGasEstimate::new(result, tip)))
    }

    pub async fn execute(&self, account: &StarknetAccount, nonce: Felt) -> Result<InvokeTransactionResult, Error> {
        let result = account.execute_v3(self.to_vec()).nonce(nonce).send().await?;

        Ok(result)
    }

    pub fn merge(&mut self, other: &Calls) {
        self.0.extend(other.0.clone());
    }

    pub fn push(&mut self, other: Call) {
        self.0.push(other)
    }

    pub fn as_transaction(&self, sender: Felt, nonce: Felt, tip: u64) -> BroadcastedTransaction {
        BroadcastedTransaction::Invoke(BroadcastedInvokeTransactionV3 {
            sender_address: sender,
            calldata: CalldataBuilder::new().encode(&self.0).build(),

            signature: vec![],
            nonce,
            resource_bounds: ResourceBoundsMapping {
                l1_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
                l1_data_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
                l2_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
            },
            tip,
            paymaster_data: vec![],
            account_deployment_data: vec![],
            nonce_data_availability_mode: DataAvailabilityMode::L1,
            fee_data_availability_mode: DataAvailabilityMode::L1,
            is_query: true,
        })
    }

    pub fn as_execute_from_outside_call(&self, caller_address: Felt, to: StarknetAccount, to_private_key: Felt, time_bounds: TimeBounds) -> Call {
        let to_address = to.address().clone();
        // Create execute_from_outside message
        let execute_from_outside_message = ExecuteFromOutsideMessage::new(
            PaymasterVersion::V1,
            ExecuteFromOutsideParameters {
                chain_id: ChainID::from_felt(to.chain_id()).unwrap(),
                caller: caller_address,
                nonce: Felt::from(Uuid::new_v4().to_u128_le()),
                calls: self.clone(),
                time_bounds,
            },
        );

        // Convert to typed data for signing
        let typed_data = execute_from_outside_message.clone().to_typed_data().unwrap();

        // Sign the message with the gas tank's private key
        let message_hash = typed_data.message_hash(to_address).unwrap();
        let signing_key = SigningKey::from_secret_scalar(to_private_key);
        let signature = signing_key.sign(&message_hash).unwrap();

        // Create the execute_from_outside call
        return execute_from_outside_message.to_call(to.address(), &vec![signature.r, signature.s]);
    }
}

#[derive(Debug)]
pub struct EstimatedCalls {
    calls: Calls,
    estimate: TransactionGasEstimate,
}

impl EstimatedCalls {
    pub fn estimate(&self) -> TransactionGasEstimate {
        self.estimate.clone()
    }

    pub async fn execute(&self, account: &StarknetAccount, nonce: Felt) -> Result<InvokeTransactionResult, Error> {
        let result = account
            .execute_v3(self.calls.to_vec())
            .nonce(nonce)
            .l1_gas(self.estimate.l1_gas_consumed())
            .l1_gas_price(self.estimate.l1_gas_price()?)
            .l2_gas(self.estimate.l2_gas_consumed())
            .l2_gas_price(self.estimate.l2_gas_price()?)
            .l1_data_gas(self.estimate.l1_data_gas_consumed())
            .l1_data_gas_price(self.estimate.l1_data_gas_price()?)
            .tip(self.estimate.tip())
            .send()
            .await;

        match &result {
            Err(AccountError::Provider(e @ ProviderError::RateLimited)) => {
                error!("{}", e);
            },
            Err(AccountError::Provider(e @ ProviderError::ArrayLengthMismatch)) => {
                error!("{}", e);
            },
            Err(AccountError::Provider(ProviderError::Other(error))) => {
                error!("{}", error);
            },
            _ => {},
        };

        Ok(result?)
    }
}
