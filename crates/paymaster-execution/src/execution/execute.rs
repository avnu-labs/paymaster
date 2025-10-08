use paymaster_prices::math::convert_strk_to_token;
use paymaster_starknet::transaction::{CalldataBuilder, Calls, EstimatedCalls, ExecuteFromOutsideMessage, TokenTransfer};
use paymaster_starknet::Signature;
use starknet::core::types::{Call, Felt, InvokeTransactionResult, TypedData};
use starknet::macros::selector;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::execution::deploy::DeploymentParameters;
use crate::execution::ExecutionParameters;
use crate::{Client, Error};

#[derive(Debug, Hash)]
pub enum ExecutableTransactionParameters {
    Deploy {
        deployment: DeploymentParameters,
    },
    Invoke {
        invoke: ExecutableInvokeParameters,
    },
    DeployAndInvoke {
        deployment: DeploymentParameters,
        invoke: ExecutableInvokeParameters,
    },
}

impl ExecutableTransactionParameters {
    pub fn get_unique_identifier(&self) -> u64 {
        match self {
            ExecutableTransactionParameters::Deploy { deployment } => deployment.get_unique_identifier(),
            ExecutableTransactionParameters::Invoke { invoke } => invoke.get_unique_identifier(),
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.get_unique_identifier(),
        }
    }
}

#[derive(Debug, Hash)]
pub struct ExecutableInvokeParameters {
    user: Felt,
    signature: Signature,

    message: ExecuteFromOutsideMessage,
}

impl ExecutableInvokeParameters {
    pub fn new(user: Felt, typed_data: TypedData, signature: Signature) -> Result<Self, Error> {
        Ok(Self {
            user,
            signature,

            message: ExecuteFromOutsideMessage::from_typed_data(&typed_data)?,
        })
    }

    fn find_gas_token_transfer(&self, forwarder: Felt) -> Result<TokenTransfer, Error> {
        let last_call = self.message.calls().last().ok_or(Error::InvalidTypedData)?;
        if last_call.selector != selector!("transfer") {
            return Err(Error::InvalidTypedData);
        }

        let transfer_recipient = last_call.calldata.first().ok_or(Error::InvalidTypedData)?;
        if *transfer_recipient != forwarder {
            return Err(Error::InvalidTypedData);
        }

        Ok(TokenTransfer::new(
            last_call.to,
            *transfer_recipient,
            *last_call.calldata.get(1).ok_or(Error::InvalidTypedData)?,
        ))
    }

    pub fn get_unique_identifier(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.user.hash(&mut hasher);
        self.message.nonce().hash(&mut hasher);
        hasher.finish()
    }
}

/// Paymaster transaction that contains the parameters to execute the transaction on Starknet
pub struct ExecutableTransaction {
    /// The forwarder to use when executing the transaction
    pub forwarder: Felt,

    /// Gas fee recipient to use when executing the transaction
    pub gas_tank_address: Felt,

    /// Parameters of the transaction which should come out from the response of the [`buildTransaction`] endpoint
    pub transaction: ExecutableTransactionParameters,

    /// Execution parameters which should come out from the response of the [`buildTransaction`] endpoint
    pub parameters: ExecutionParameters,
}

impl ExecutableTransaction {
    /// Estimate a sponsored transaction which is a transaction that will be paid by the relayer
    pub async fn estimate_sponsored_transaction(self, client: &Client, sponsor_metadata: Vec<Felt>) -> Result<EstimatedExecutableTransaction, Error> {
        let calls = self.build_sponsored_calls(sponsor_metadata);

        let estimated_calls = client.estimate(&calls, self.parameters.tip()).await?;
        let fee_estimate = estimated_calls.estimate();

        // We recompute the real estimate fee. Validation step is not included in the fee estimate
        let paid_fee_in_strk = self.compute_paid_fee(client, Felt::from(fee_estimate.overall_fee)).await?;
        let final_fee_estimate = fee_estimate.update_overall_fee(paid_fee_in_strk);

        let estimated_final_calls = calls.with_estimate(final_fee_estimate);
        Ok(EstimatedExecutableTransaction(estimated_final_calls))
    }

    /// Estimate an unsponsored transaction which is a transaction that will be paid by the user
    pub async fn estimate_transaction(self, client: &Client) -> Result<EstimatedExecutableTransaction, Error> {
        let gas_token_transfer = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke.find_gas_token_transfer(self.forwarder)?,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.find_gas_token_transfer(self.forwarder)?,
            _ => return Err(Error::InvalidTypedData),
        };

        let calls = self.build_calls(gas_token_transfer);

        let estimated_calls = client.estimate(&calls, self.parameters.tip()).await?;
        let fee_estimate = estimated_calls.estimate();

        // We recompute the real estimate fee. Validation step is not included in the fee estimate
        let paid_fee_in_strk = self.compute_paid_fee(client, Felt::from(fee_estimate.overall_fee)).await?;
        let final_fee_estimate = fee_estimate.update_overall_fee(paid_fee_in_strk);

        let token_price = client.price.fetch_token(gas_token_transfer.token()).await?;
        let paid_fee_in_token = convert_strk_to_token(&token_price, paid_fee_in_strk, true)?;

        // Check that the user has approved enough token to cover the real cost. The value approved by the user
        // is extracted from the signed execute from outside message.
        if paid_fee_in_token > gas_token_transfer.amount() {
            return Err(Error::MaxAmountTooLow(paid_fee_in_token.to_hex_string()));
        }

        let fee_transfer = TokenTransfer::new(gas_token_transfer.token(), self.gas_tank_address, paid_fee_in_token);
        let final_calls = self.build_calls(fee_transfer);
        let estimated_final_calls = final_calls.with_estimate(final_fee_estimate);

        Ok(EstimatedExecutableTransaction(estimated_final_calls))
    }

    // Compute the fee that will be paid upon execution
    async fn compute_paid_fee(&self, client: &Client, base_estimate: Felt) -> Result<Felt, Error> {
        match &self.transaction {
            ExecutableTransactionParameters::Deploy { .. } => Ok(client.compute_paid_fee_in_strk(base_estimate)),
            ExecutableTransactionParameters::Invoke { invoke, .. } => client.compute_paid_fee_with_overhead_in_strk(invoke.user, base_estimate).await,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => client.compute_paid_fee_with_overhead_in_strk(invoke.user, base_estimate).await,
        }
    }

    // Build the calls that needs to be performed
    fn build_calls(&self, fee_transfer: TokenTransfer) -> Calls {
        let calls = [self.build_deploy_call(), self.build_execute_call(fee_transfer)]
            .into_iter()
            .flatten()
            .collect();

        Calls::new(calls)
    }

    // Build the calls that needs to be performed
    fn build_sponsored_calls(&self, sponsor_metadata: Vec<Felt>) -> Calls {
        let calls = [self.build_deploy_call(), self.build_sponsored_execute_call(sponsor_metadata)]
            .into_iter()
            .flatten()
            .collect();

        Calls::new(calls)
    }

    fn build_deploy_call(&self) -> Option<Call> {
        match &self.transaction {
            ExecutableTransactionParameters::Deploy { deployment, .. } => Some(deployment.as_call()),
            ExecutableTransactionParameters::DeployAndInvoke { deployment, .. } => Some(deployment.as_call()),
            _ => None,
        }
    }

    fn build_execute_call(&self, fee_transfer: TokenTransfer) -> Option<Call> {
        let invoke = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke,
            _ => return None,
        };

        let execute_from_outside_call = invoke.message.to_call(invoke.user, &invoke.signature);

        Some(Call {
            to: self.forwarder,
            selector: selector!("execute"),
            calldata: CalldataBuilder::new()
                .encode(&execute_from_outside_call)
                .encode(&fee_transfer.token())
                .encode(&fee_transfer.amount())
                .encode(&Felt::ZERO)
                .build(),
        })
    }

    fn build_sponsored_execute_call(&self, sponsor_metadata: Vec<Felt>) -> Option<Call> {
        let invoke = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke,
            _ => return None,
        };

        let execute_from_outside_call = invoke.message.to_call(invoke.user, &invoke.signature);

        Some(Call {
            to: self.forwarder,
            selector: selector!("execute_sponsored"),
            calldata: CalldataBuilder::new()
                .encode(&execute_from_outside_call)
                .encode(&sponsor_metadata)
                .build(),
        })
    }
}

/// Paymaster executable transaction that can be sent to Starknet
#[derive(Debug)]
pub struct EstimatedExecutableTransaction(EstimatedCalls);

impl EstimatedExecutableTransaction {
    pub async fn execute(self, client: &Client) -> Result<InvokeTransactionResult, Error> {
        let result = client.execute(&self.0).await?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use paymaster_starknet::transaction::{Calls, TokenTransfer};
    use rand::Rng;
    use starknet::accounts::{Account, AccountFactory};
    use starknet::core::types::Felt;
    use starknet::signers::SigningKey;

    use crate::execution::build::{InvokeParameters, Transaction, TransactionParameters};
    use crate::execution::deploy::DeploymentParameters;
    use crate::execution::execute::{ExecutableInvokeParameters, ExecutableTransaction, ExecutableTransactionParameters};
    use crate::execution::{ExecutionParameters, FeeMode, TipPriority};
    use crate::testing::transaction::{an_eth_approve, an_eth_transfer};
    use crate::testing::{StarknetTestEnvironment, TestEnvironment};

    #[tokio::test]
    async fn execute_deploy_transaction_sponsored_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);

        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;
        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(
                &account,
                &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::from(1e16 as u128)),
            )
            .await;

        let deployment = DeploymentParameters {
            version: 2,
            address: new_account_address,
            class_hash: new_account.class_hash(),
            unique: Felt::ZERO,
            salt,
            calldata: new_account.calldata(),
            sigdata: None,
        };

        let client = test.default_client();

        let transaction = ExecutableTransaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            gas_tank_address: StarknetTestEnvironment::FORWARDER,

            transaction: ExecutableTransactionParameters::Deploy { deployment },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Sponsored { tip: TipPriority::Normal },
                time_bounds: None,
            },
        };

        let estimate = transaction.estimate_sponsored_transaction(&client, vec![]).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }

    // TODO: fix starknet-devnet
    // #[tokio::test]
    async fn execute_invoke_transaction_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);

        let user = StarknetTestEnvironment::ACCOUNT_ARGENT_1;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,

            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: user.address,
                    calls: Calls::new(vec![an_eth_transfer(account.address(), Felt::ONE, test.starknet.chain_id())]),
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                    tip: TipPriority::Normal,
                },
                time_bounds: None,
            },
        };

        let client = test.default_client();

        let estimated_transaction = transaction.estimate(&client).await.unwrap();
        let versioned_estimated_transaction = estimated_transaction.resolve_version(&client).await.unwrap();

        let typed_data = versioned_estimated_transaction
            .to_execute_from_outside()
            .to_typed_data()
            .unwrap();
        let message_hash = typed_data.message_hash(user.address).unwrap();
        let signed_message = SigningKey::from_secret_scalar(user.private_key).sign(&message_hash).unwrap();

        let transaction = ExecutableTransaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            gas_tank_address: StarknetTestEnvironment::FORWARDER,

            transaction: ExecutableTransactionParameters::Invoke {
                invoke: ExecutableInvokeParameters::new(user.address, typed_data, vec![signed_message.r, signed_message.s]).unwrap(),
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                    tip: TipPriority::Normal,
                },
                time_bounds: None,
            },
        };

        let estimate = transaction.estimate_transaction(&client).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }

    // TODO: fix starknet-devnet
    // #[tokio::test]
    async fn execute_deploy_and_invoke_transaction_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);

        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;
        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(
                &account,
                &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::from(1e16 as u128)),
            )
            .await;

        let deployment = DeploymentParameters {
            version: 2,
            address: new_account_address,
            class_hash: new_account.class_hash(),
            unique: Felt::ZERO,
            salt,
            calldata: new_account.calldata(),
            sigdata: None,
        };

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,

            transaction: TransactionParameters::DeployAndInvoke {
                deployment: deployment.clone(),
                invoke: InvokeParameters {
                    user_address: new_account_address,
                    calls: Calls::new(vec![an_eth_approve(account.address(), Felt::ZERO, test.starknet.chain_id())]),
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                    tip: TipPriority::Normal,
                },
                time_bounds: None,
            },
        };

        let client = test.default_client();

        let estimated_transaction = transaction.estimate(&client).await.unwrap();
        let versioned_estimated_transaction = estimated_transaction.resolve_version(&client).await.unwrap();

        let typed_data = versioned_estimated_transaction
            .to_execute_from_outside()
            .to_typed_data()
            .unwrap();
        let message_hash = typed_data.message_hash(new_account_address).unwrap();
        let signed_message = SigningKey::from_secret_scalar(Felt::ONE).sign(&message_hash).unwrap();

        let transaction = ExecutableTransaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            gas_tank_address: StarknetTestEnvironment::FORWARDER,

            transaction: ExecutableTransactionParameters::DeployAndInvoke {
                deployment,
                invoke: ExecutableInvokeParameters::new(new_account_address, typed_data, vec![signed_message.r, signed_message.s]).unwrap(),
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                    tip: TipPriority::Normal,
                },
                time_bounds: None,
            },
        };

        let estimate = transaction.estimate_transaction(&client).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }
}
