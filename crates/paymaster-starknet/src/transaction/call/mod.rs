use std::ops::Deref;

use serde::{Deserialize, Serialize};
use starknet::accounts::{Account, AccountError, ConnectedAccount};
use starknet::core::types::{
    BlockId, BlockTag, BroadcastedInvokeTransaction, BroadcastedInvokeTransactionV3, BroadcastedTransaction, Call, DataAvailabilityMode, Felt, InvokeTransactionResult,
    ResourceBounds, ResourceBoundsMapping, StarknetError,
};
use starknet::providers::jsonrpc::{HttpTransportError, JsonRpcClientError};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::SigningKey;

use crate::client::NoAvailableEndpoint;
use crate::transaction::{ExecuteFromOutsideMessage, ExecuteFromOutsideParameters, PaymasterVersion, PrivateProofData, TimeBounds, TransactionGasEstimate};
use crate::{ChainID, Error, StarknetAccount, UncertainSubmission};

mod calldata;
pub use calldata::{AsCalldata, CalldataBuilder, SequentialCalldataDecoder};
mod server_action;
pub use server_action::{parse_server_actions, ServerAction, ServerActionError};
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
        EstimatedCalls {
            calls: self,
            estimate,
            proof_data: None,
        }
    }

    pub fn with_estimate_and_proof(self, estimate: TransactionGasEstimate, proof_data: PrivateProofData) -> EstimatedCalls {
        EstimatedCalls {
            calls: self,
            estimate,
            proof_data: Some(proof_data),
        }
    }

    pub async fn estimate(&self, account: &StarknetAccount, tip: Option<u64>) -> Result<EstimatedCalls, Error> {
        let tip = match tip {
            None => {
                let block = account
                    .provider()
                    .get_block_with_txs(BlockId::Tag(BlockTag::Latest), None)
                    .await?;
                block.median_tip()
            },
            Some(tip) => tip,
        };

        let result = account.execute_v3(self.to_vec()).tip(tip).estimate_fee().await?;

        Ok(self.clone().with_estimate(TransactionGasEstimate::new(result, tip)))
    }

    pub async fn estimate_with_proof(&self, account: &StarknetAccount, tip: Option<u64>, proof_data: &PrivateProofData) -> Result<EstimatedCalls, Error> {
        let tip = match tip {
            None => {
                let block = account
                    .provider()
                    .get_block_with_txs(BlockId::Tag(BlockTag::Latest), None)
                    .await?;
                block.median_tip()
            },
            Some(tip) => tip,
        };

        let result = account
            .execute_v3(self.to_vec())
            .tip(tip)
            .proof_facts(proof_data.proof_facts.clone())
            .proof(proof_data.proof.clone())
            .estimate_fee()
            .await?;

        Ok(self
            .clone()
            .with_estimate_and_proof(TransactionGasEstimate::new(result, tip), proof_data.clone()))
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
        BroadcastedTransaction::Invoke(BroadcastedInvokeTransaction {
            broadcasted_invoke_txn_v3: BroadcastedInvokeTransactionV3 {
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
                proof_facts: None,
                is_query: true,
            },
            proof: None,
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
    proof_data: Option<PrivateProofData>,
}

impl EstimatedCalls {
    pub fn estimate(&self) -> TransactionGasEstimate {
        self.estimate.clone()
    }

    /// Override the maximum L2 gas amount the declared gas bound will be capped to. See
    /// [`TransactionGasEstimate::with_max_l2_gas_amount`].
    pub fn with_max_l2_gas_amount(mut self, max_l2_gas_amount: u64) -> Self {
        self.estimate = self.estimate.with_max_l2_gas_amount(max_l2_gas_amount);
        self
    }

    pub async fn execute(&self, account: &StarknetAccount, nonce: Felt) -> Result<InvokeTransactionResult, Error> {
        // Reject before broadcast when the transaction cannot fit under the sequencer L2 gas cap, so we
        // surface a clear error instead of a paid out-of-gas revert.
        self.estimate.check_l2_gas_within_cap()?;

        let mut execution = account
            .execute_v3(self.calls.to_vec())
            .nonce(nonce)
            .l1_gas(self.estimate.l1_gas_consumed())
            .l1_gas_price(self.estimate.l1_gas_price()?)
            .l2_gas(self.estimate.l2_gas_consumed())
            .l2_gas_price(self.estimate.l2_gas_price()?)
            .l1_data_gas(self.estimate.l1_data_gas_consumed())
            .l1_data_gas_price(self.estimate.l1_data_gas_price()?)
            .tip(self.estimate.tip());

        if let Some(proof_data) = &self.proof_data {
            execution = execution
                .proof_facts(proof_data.proof_facts.clone())
                .proof(proof_data.proof.clone());
        }

        let execution = execution.prepared().map_err(|e| Error::Internal(e.to_string()))?;
        let transaction_hash = execution.transaction_hash(false);

        match execution.send().await {
            Err(AccountError::Provider(source)) if !is_refusal(&source) => {
                let source = Box::new(source);
                Err(UncertainSubmission { transaction_hash, source }.into())
            },
            result => Ok(result?),
        }
    }
}

/// Whether a submission error proves the transaction was not accepted: a refusal declared by
/// `starknet_addInvokeTransaction`, or a request that never reached a node. Any other error (timeout, duplicate
/// in the mempool, unexpected or undeclared error...) may hide a transaction accepted by the sequencer.
fn is_refusal(error: &ProviderError) -> bool {
    match error {
        ProviderError::StarknetError(error) => matches!(
            error,
            StarknetError::ValidationFailure(_)
                | StarknetError::InvalidTransactionNonce(_)
                | StarknetError::InsufficientResourcesForValidate
                | StarknetError::InsufficientAccountBalance
                | StarknetError::FeeBelowMinimum
                | StarknetError::ReplacementTransactionUnderpriced
                | StarknetError::NonAccount
                | StarknetError::UnsupportedTxVersion
                | StarknetError::InvalidProof
        ),
        ProviderError::Other(error) => {
            let error = error.as_any();
            error.is::<NoAvailableEndpoint>()
                || matches!(
                    error.downcast_ref::<JsonRpcClientError<HttpTransportError>>(),
                    Some(JsonRpcClientError::TransportError(HttpTransportError::Reqwest(error))) if error.is_connect()
                )
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::{ContractErrorData, ContractExecutionError, Felt, StarknetError};
    use starknet::providers::jsonrpc::{HttpTransport, HttpTransportError, JsonRpcClientError};
    use starknet::providers::{JsonRpcClient, Provider, ProviderError, Url};

    use super::is_refusal;
    use crate::client::NoAvailableEndpoint;

    #[tokio::test]
    async fn refusals_are_recognized() {
        let unreachable_node = JsonRpcClient::new(HttpTransport::new(Url::parse("http://127.0.0.1:1").unwrap()));
        let errors = [
            ProviderError::StarknetError(StarknetError::ValidationFailure("invalid signature".to_string())),
            ProviderError::StarknetError(StarknetError::InvalidTransactionNonce("invalid nonce".to_string())),
            ProviderError::StarknetError(StarknetError::InsufficientResourcesForValidate),
            ProviderError::StarknetError(StarknetError::InsufficientAccountBalance),
            ProviderError::StarknetError(StarknetError::FeeBelowMinimum),
            ProviderError::StarknetError(StarknetError::ReplacementTransactionUnderpriced),
            ProviderError::StarknetError(StarknetError::NonAccount),
            ProviderError::StarknetError(StarknetError::UnsupportedTxVersion),
            ProviderError::StarknetError(StarknetError::InvalidProof),
            ProviderError::Other(Box::new(NoAvailableEndpoint)),
            unreachable_node.block_number().await.unwrap_err(),
        ];

        for error in errors {
            assert!(is_refusal(&error), "{error:?}");
        }
    }

    #[test]
    fn other_errors_are_uncertain() {
        let malformed_response = serde_json::from_str::<Felt>("").unwrap_err();
        let errors = [
            ProviderError::Other(Box::new(JsonRpcClientError::<HttpTransportError>::JsonError(malformed_response))),
            ProviderError::RateLimited,
            ProviderError::StarknetError(StarknetError::DuplicateTx),
            ProviderError::StarknetError(StarknetError::UnexpectedError("gateway timeout".to_string())),
            ProviderError::StarknetError(StarknetError::ContractError(ContractErrorData {
                revert_error: ContractExecutionError::Message("reverted".to_string()),
            })),
        ];

        for error in errors {
            assert!(!is_refusal(&error), "{error:?}");
        }
    }
}
