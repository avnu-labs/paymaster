use paymaster_prices::math::convert_strk_to_token;
use paymaster_starknet::constants::Token;
use paymaster_starknet::transaction::{
    parse_server_actions, CalldataBuilder, Calls, EstimatedCalls, ExecuteFromOutsideMessage, PrivateProofData, SequentialCalldataDecoder, ServerAction, TokenTransfer,
};
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
    DirectInvoke {
        invoke: ExecutableDirectInvokeParameters,
    },
    ApplyAction {
        apply_action: ExecutableApplyActionParameters,
    },
    InvokeAndApplyAction {
        invoke: ExecutableInvokeParameters,
        apply_action: ExecutableApplyActionParameters,
    },
}

impl ExecutableTransactionParameters {
    pub fn get_unique_identifier(&self) -> u64 {
        match self {
            ExecutableTransactionParameters::Deploy { deployment } => deployment.get_unique_identifier(),
            ExecutableTransactionParameters::Invoke { invoke } => invoke.get_unique_identifier(),
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.get_unique_identifier(),
            ExecutableTransactionParameters::DirectInvoke { invoke } => invoke.get_unique_indentifier(),
            ExecutableTransactionParameters::ApplyAction { apply_action } => apply_action.get_unique_identifier(),
            ExecutableTransactionParameters::InvokeAndApplyAction { invoke, apply_action } => {
                let mut hasher = DefaultHasher::new();
                invoke.user.hash(&mut hasher);
                invoke.message.nonce().hash(&mut hasher);
                apply_action.apply_actions_call.calldata.hash(&mut hasher);
                apply_action.proof_data.hash(&mut hasher);
                hasher.finish()
            },
        }
    }

    pub fn extract_proof_data(&self) -> Option<PrivateProofData> {
        match self {
            ExecutableTransactionParameters::ApplyAction { apply_action } => Some(apply_action.proof_data.clone()),
            ExecutableTransactionParameters::InvokeAndApplyAction { apply_action, .. } => Some(apply_action.proof_data.clone()),
            _ => None,
        }
    }

    pub fn is_private(&self) -> bool {
        matches!(self, Self::ApplyAction { .. } | Self::InvokeAndApplyAction { .. })
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

#[derive(Debug, Hash)]
pub struct ExecutableApplyActionParameters {
    pub apply_actions_call: Call,
    pub proof_data: PrivateProofData,
}

impl ExecutableApplyActionParameters {
    pub fn new(apply_actions_call: Call, proof: String, proof_facts: Vec<Felt>) -> Self {
        Self {
            apply_actions_call,
            proof_data: PrivateProofData { proof, proof_facts },
        }
    }

    pub fn get_unique_identifier(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.apply_actions_call.calldata.hash(&mut hasher);
        self.proof_data.hash(&mut hasher);
        hasher.finish()
    }

    fn find_gas_token_transfer(&self, forwarder: Felt) -> Result<TokenTransfer, Error> {
        let actions = parse_server_actions(&self.apply_actions_call.calldata).map_err(|e| Error::CalldataParsing(e.to_string()))?;

        let (transfer_token, transfer_amount) = actions
            .iter()
            .find_map(|action| match action {
                ServerAction::TransferTo { to_addr, token, amount } if *to_addr == forwarder => Some((*token, *amount)),
                _ => None,
            })
            .ok_or(Error::MissingFeeTransferTo)?;

        Ok(TokenTransfer::new(transfer_token, forwarder, Felt::from(transfer_amount)))
    }
}

#[derive(Debug, Hash)]
pub struct ExecutableDirectInvokeParameters {
    pub user: Felt,
    pub execute_from_outside_call: Call,
}

impl ExecutableDirectInvokeParameters {
    pub fn get_unique_indentifier(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.user.hash(&mut hasher);
        self.execute_from_outside_call.calldata.hash(&mut hasher);

        hasher.finish()
    }

    /// Extract gas transfer from a raw execute_from_outside call
    ///
    /// The execute_from_outside_call has calldata structure:
    /// [caller, nonce, execute_after, execute_before, calls_array...]
    /// where calls_array is [num_calls, ...encoded_calls]
    /// and each call is [to, selector, calldata_len, ...calldata]
    ///
    /// For non-sponsored transactions, the last call should be a transfer of gas token to the forwarder.
    fn find_gas_token_transfer(&self, forwarder: Felt) -> Result<TokenTransfer, Error> {
        let calls: Vec<Felt> = self.execute_from_outside_call.calldata.iter().skip(5).cloned().collect();

        let decoder = SequentialCalldataDecoder::new(&calls)?;
        let last_call = decoder.last().ok_or(Error::InvalidTypedData)?;

        // Validate the last call is a transfer to the forwarder
        if last_call.selector != selector!("transfer") {
            return Err(Error::InvalidTypedData);
        }

        if last_call.calldata.len() != 3 {
            return Err(Error::InvalidTypedData);
        }

        let recipient = last_call.calldata.first().ok_or(Error::InvalidTypedData)?;

        if *recipient != forwarder {
            return Err(Error::InvalidTypedData);
        }

        let amount = last_call.calldata.get(1).ok_or(Error::InvalidTypedData)?;

        Ok(TokenTransfer::new(last_call.to, forwarder, *amount))
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

    /// Whitelisted privacy pool contract address
    pub privacy_pool: Felt,

    /// Pool's collect_fee cost in STRK, charged on top of gas for private transactions
    pub privacy_pool_fee_amount: u128,
}

impl ExecutableTransaction {
    /// Estimate a sponsored transaction which is a transaction that will be paid by the relayer
    pub async fn estimate_sponsored_transaction(self, client: &Client, sponsor_metadata: Vec<Felt>) -> Result<EstimatedExecutableTransaction, Error> {
        let proof_data = self.transaction.extract_proof_data();

        let (calls, estimated_calls) = match &self.transaction {
            ExecutableTransactionParameters::ApplyAction { apply_action } => {
                let calls = self.build_private_sponsored_calls(None, apply_action, sponsor_metadata)?;
                let estimated = client
                    .estimate_for_private(&calls, self.parameters.tip(), proof_data.as_ref().expect("ApplyAction must have proof_data"))
                    .await?;
                (calls, estimated)
            },
            ExecutableTransactionParameters::InvokeAndApplyAction { invoke, apply_action } => {
                let calls = self.build_private_sponsored_calls(Some(invoke), apply_action, sponsor_metadata)?;
                let estimated = client
                    .estimate_for_private(&calls, self.parameters.tip(), proof_data.as_ref().expect("InvokeAndApplyAction must have proof_data"))
                    .await?;
                (calls, estimated)
            },
            _ => {
                let calls = self.build_sponsored_calls(sponsor_metadata);
                let estimated = client.estimate(&calls, self.parameters.tip()).await.map_err(|e| {
                    tracing::warn!(error = %e, "Sponsored transaction estimation failed");
                    e
                })?;
                (calls, estimated)
            },
        };

        let fee_estimate = estimated_calls.estimate();

        // We recompute the real estimate fee. Validation step is not included in the fee estimate
        let paid_fee_in_strk = self
            .compute_paid_fee(client, Felt::from(fee_estimate.overall_fee))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, overall_fee = %fee_estimate.overall_fee, "Sponsored transaction fee computation failed");
                e
            })?;
        let final_fee_estimate = fee_estimate.update_overall_fee(paid_fee_in_strk);

        // Validate pool fee transfer for private sponsored transactions
        if self.privacy_pool_fee_amount > 0 {
            if let Some(apply_action) = match &self.transaction {
                ExecutableTransactionParameters::ApplyAction { apply_action } => Some(apply_action),
                ExecutableTransactionParameters::InvokeAndApplyAction { apply_action, .. } => Some(apply_action),
                _ => None,
            } {
                let transfer = apply_action.find_gas_token_transfer(self.forwarder)?;
                let token_price = client.price.fetch_token(transfer.token()).await?;
                let pool_fee_in_token = convert_strk_to_token(&token_price, Felt::from(self.privacy_pool_fee_amount), true)?;
                if pool_fee_in_token > transfer.amount() {
                    return Err(Error::PoolFeeTooLow(pool_fee_in_token.to_hex_string()));
                }
            }
        }

        let estimated_final_calls = if let Some(proof_data) = proof_data {
            calls.with_estimate_and_proof(final_fee_estimate, proof_data)
        } else {
            calls.with_estimate(final_fee_estimate)
        };
        Ok(EstimatedExecutableTransaction(estimated_final_calls))
    }

    pub async fn estimate_transaction(self, client: &Client) -> Result<EstimatedExecutableTransaction, Error> {
        if self.transaction.is_private() {
            self.estimate_private_transaction(client).await
        } else {
            self.estimate_standard_transaction(client).await
        }
    }

    async fn estimate_standard_transaction(self, client: &Client) -> Result<EstimatedExecutableTransaction, Error> {
        let transfer = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke.find_gas_token_transfer(self.forwarder)?,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.find_gas_token_transfer(self.forwarder)?,
            ExecutableTransactionParameters::DirectInvoke { invoke, .. } => invoke.find_gas_token_transfer(self.forwarder)?,
            _ => return Err(Error::InvalidTypedData),
        };

        let calls = self.build_calls(transfer);

        let estimated_calls = client.estimate(&calls, self.parameters.tip()).await.map_err(|e| {
            tracing::warn!(error = %e, "Transaction estimation failed");
            e
        })?;
        let fee_estimate = estimated_calls.estimate();

        let paid_fee_in_strk = self
            .compute_paid_fee(client, Felt::from(fee_estimate.overall_fee))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, overall_fee = %fee_estimate.overall_fee, "Transaction fee computation failed");
                e
            })?;
        let final_fee_estimate = fee_estimate.update_overall_fee(paid_fee_in_strk);

        let token_price = client.price.fetch_token(transfer.token()).await?;
        let paid_fee_in_token = convert_strk_to_token(&token_price, paid_fee_in_strk, true)?;

        if paid_fee_in_token > transfer.amount() {
            return Err(Error::MaxAmountTooLow(paid_fee_in_token.to_hex_string()));
        }

        let fee_transfer = TokenTransfer::new(transfer.token(), self.gas_tank_address, paid_fee_in_token);
        let final_calls = self.build_calls(fee_transfer);
        let estimated_final_calls = final_calls.with_estimate(final_fee_estimate);

        Ok(EstimatedExecutableTransaction(estimated_final_calls))
    }

    async fn estimate_private_transaction(&self, client: &Client) -> Result<EstimatedExecutableTransaction, Error> {
        let (invoke, apply_action) = match &self.transaction {
            ExecutableTransactionParameters::ApplyAction { apply_action } => (None, apply_action),
            ExecutableTransactionParameters::InvokeAndApplyAction { invoke, apply_action } => (Some(invoke), apply_action),
            _ => return Err(Error::MissingFeeTransferTo),
        };
        let transfer = apply_action.find_gas_token_transfer(self.forwarder)?;

        let calls = self.build_private_calls(invoke, apply_action, &transfer)?;

        let estimated_calls = client
            .estimate_for_private(&calls, self.parameters.tip(), &apply_action.proof_data)
            .await?;
        let fee_estimate = estimated_calls.estimate();

        let gas_fee_in_strk = self.compute_paid_fee(client, Felt::from(fee_estimate.overall_fee)).await?;
        let gas_estimate = fee_estimate.update_overall_fee(gas_fee_in_strk);
        let required_fee_in_strk = gas_fee_in_strk + Felt::from(self.privacy_pool_fee_amount);

        let token_price = client.price.fetch_token(transfer.token()).await?;
        let required_fee_in_token = convert_strk_to_token(&token_price, required_fee_in_strk, true)?;

        if required_fee_in_token > transfer.amount() {
            return Err(Error::MaxAmountTooLow(required_fee_in_token.to_hex_string()));
        }

        // Rebuild calls with the real fee amount so the forwarder only transfers the actual cost
        let real_transfer = TokenTransfer::new(transfer.token(), self.forwarder, required_fee_in_token);
        let final_calls = self.build_private_calls(invoke, apply_action, &real_transfer)?;
        let estimated_final_calls = final_calls.with_estimate_and_proof(gas_estimate, apply_action.proof_data.clone());

        Ok(EstimatedExecutableTransaction(estimated_final_calls))
    }

    async fn compute_paid_fee(&self, client: &Client, base_estimate: Felt) -> Result<Felt, Error> {
        match &self.transaction {
            ExecutableTransactionParameters::Deploy { .. } => Ok(client.compute_paid_fee_in_strk(base_estimate)),
            ExecutableTransactionParameters::Invoke { invoke, .. } => client.compute_paid_fee_with_overhead_in_strk(invoke.user, base_estimate).await,
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => client.compute_paid_fee_with_overhead_in_strk(invoke.user, base_estimate).await,
            ExecutableTransactionParameters::DirectInvoke { invoke, .. } => client.compute_paid_fee_with_overhead_in_strk(invoke.user, base_estimate).await,
            ExecutableTransactionParameters::ApplyAction { .. } | ExecutableTransactionParameters::InvokeAndApplyAction { .. } => {
                Ok(client.compute_paid_fee_in_strk(base_estimate))
            },
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

    /// The privacy pool collects its fee in STRK via `transferFrom(forwarder, ...)`. The forwarder is
    /// never paid in STRK — the user may be paying in a different gas token (USDC, etc.) — so the
    /// relayer seeds the forwarder's STRK balance before the forwarder call. The relayer is later
    /// reimbursed through the standard gas-tank → relayers STRK rebalancing cycle.
    fn build_pool_fee_pretransfer_call(&self) -> Option<Call> {
        if self.privacy_pool_fee_amount == 0 {
            return None;
        }
        Some(TokenTransfer::new(Token::STRK_ADDRESS, self.forwarder, Felt::from(self.privacy_pool_fee_amount)).to_call())
    }

    /// Validate and build the inner calls for a private transaction (optional execute_from_outside + apply_actions)
    fn build_private_inner_calls(&self, invoke: Option<&ExecutableInvokeParameters>, apply_action: &ExecutableApplyActionParameters) -> Result<Vec<Call>, Error> {
        let apply_call = &apply_action.apply_actions_call;

        if self.privacy_pool != apply_call.to {
            return Err(Error::PrivacyPoolNotWhitelisted);
        }
        if apply_call.selector != selector!("apply_actions") {
            return Err(Error::InvalidApplyActionsSelector);
        }

        let mut calls = Vec::new();
        if let Some(invoke) = invoke {
            calls.push(invoke.message.to_call(invoke.user, &invoke.signature));
        }
        calls.push(apply_call.clone());
        Ok(calls)
    }

    /// Build calls for a private sponsored transaction using `execute_private_sponsored`
    fn build_private_sponsored_calls(
        &self,
        invoke: Option<&ExecutableInvokeParameters>,
        apply_action: &ExecutableApplyActionParameters,
        sponsor_metadata: Vec<Felt>,
    ) -> Result<Calls, Error> {
        let inner_calls = self.build_private_inner_calls(invoke, apply_action)?;

        // Extract pool fee transfer from apply_actions calldata (if pool fee > 0)
        let (pool_fee_token, pool_fee_amount) = if self.privacy_pool_fee_amount > 0 {
            let transfer = apply_action.find_gas_token_transfer(self.forwarder)?;
            (transfer.token(), transfer.amount())
        } else {
            (Felt::ZERO, Felt::ZERO)
        };

        let forwarder_call = Call {
            to: self.forwarder,
            selector: selector!("execute_private_sponsored"),
            calldata: CalldataBuilder::new()
                .encode(&inner_calls)
                .encode(&pool_fee_token)
                .encode(&pool_fee_amount)
                .encode(&Felt::ZERO)
                .encode(&sponsor_metadata)
                .build(),
        };

        let mut calls = Vec::with_capacity(2);
        if let Some(pretransfer) = self.build_pool_fee_pretransfer_call() {
            calls.push(pretransfer);
        }
        calls.push(forwarder_call);
        Ok(Calls::new(calls))
    }

    /// Build calls for a gasless private transaction using `execute_private`
    fn build_private_calls(
        &self,
        invoke: Option<&ExecutableInvokeParameters>,
        apply_action: &ExecutableApplyActionParameters,
        transfer: &TokenTransfer,
    ) -> Result<Calls, Error> {
        let inner_calls = self.build_private_inner_calls(invoke, apply_action)?;

        let forwarder_call = Call {
            to: self.forwarder,
            selector: selector!("execute_private"),
            calldata: CalldataBuilder::new()
                .encode(&inner_calls)
                .encode(&transfer.token())
                .encode(&transfer.amount())
                .encode(&Felt::ZERO)
                .build(),
        };

        let mut calls = Vec::with_capacity(2);
        if let Some(pretransfer) = self.build_pool_fee_pretransfer_call() {
            calls.push(pretransfer);
        }
        calls.push(forwarder_call);
        Ok(Calls::new(calls))
    }

    fn build_deploy_call(&self) -> Option<Call> {
        match &self.transaction {
            ExecutableTransactionParameters::Deploy { deployment, .. } => Some(deployment.as_call()),
            ExecutableTransactionParameters::DeployAndInvoke { deployment, .. } => Some(deployment.as_call()),
            _ => None,
        }
    }

    fn build_execute_call(&self, fee_transfer: TokenTransfer) -> Option<Call> {
        let execute_from_outside_call = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke.message.to_call(invoke.user, &invoke.signature),
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.message.to_call(invoke.user, &invoke.signature),
            ExecutableTransactionParameters::DirectInvoke { invoke, .. } => invoke.execute_from_outside_call.clone(),
            ExecutableTransactionParameters::ApplyAction { .. } | ExecutableTransactionParameters::InvokeAndApplyAction { .. } => return None,
            _ => return None,
        };

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
        let execute_from_outside_call = match &self.transaction {
            ExecutableTransactionParameters::Invoke { invoke, .. } => invoke.message.to_call(invoke.user, &invoke.signature),
            ExecutableTransactionParameters::DeployAndInvoke { invoke, .. } => invoke.message.to_call(invoke.user, &invoke.signature),
            ExecutableTransactionParameters::DirectInvoke { invoke, .. } => invoke.execute_from_outside_call.clone(),
            ExecutableTransactionParameters::ApplyAction { .. } | ExecutableTransactionParameters::InvokeAndApplyAction { .. } => return None,
            _ => return None,
        };

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
    use crate::execution::build::{InvokeParameters, Transaction, TransactionParameters};
    use crate::execution::deploy::DeploymentParameters;
    use crate::execution::execute::{ExecutableInvokeParameters, ExecutableTransaction, ExecutableTransactionParameters};
    use crate::execution::{ExecutionParameters, FeeMode, TipPriority};
    use crate::testing::transaction::{an_eth_approve, an_eth_transfer};
    use crate::testing::{StarknetTestEnvironment, TestEnvironment};
    use crate::ExecutableDirectInvokeParameters;
    use paymaster_starknet::transaction::{Calls, TokenTransfer};
    use rand::Rng;
    use starknet::accounts::{Account, AccountFactory};
    use starknet::core::types::{Call, Felt};
    use starknet::macros::{felt, selector};
    use starknet::signers::SigningKey;

    #[test]
    fn extract_gas_transfer_from_raw_call_works() {
        let forwarder = felt!("0x123");
        let token = felt!("0x456");
        let amount = felt!("0x789");

        // Build a simple execute_from_outside call with one user call + gas transfer
        // Structure: [caller, nonce, execute_after, execute_before, num_calls, call1..., call2...]
        let calldata = vec![
            felt!("0x1"), // caller
            felt!("0x2"), // nonce
            felt!("0x3"), // execute_after
            felt!("0x4"), // execute_before
            Felt::TWO,    // num_calls = 2
            // First call (user's transfer)
            felt!("0xAAA"),        // to
            selector!("transfer"), // selector
            Felt::THREE,           // calldata_len
            felt!("0xBBB"),        // recipient
            felt!("0xCCC"),        // amount_low
            Felt::ZERO,            // amount_high
            // Second call (gas transfer to forwarder)
            token,                 // to (token address)
            selector!("transfer"), // selector
            Felt::THREE,           // calldata_len
            forwarder,             // recipient (forwarder)
            amount,                // amount_low
            Felt::ZERO,            // amount_high
        ];

        let parameters = ExecutableDirectInvokeParameters {
            user: Felt::ZERO,
            execute_from_outside_call: Call {
                to: felt!("0x999"),
                selector: selector!("execute_from_outside"),
                calldata,
            },
        };

        let result = parameters.find_gas_token_transfer(forwarder);
        assert!(result.is_ok());

        let transfer = result.unwrap();
        assert_eq!(transfer.token(), token);
        assert_eq!(transfer.recipient(), forwarder);
        assert_eq!(transfer.amount(), amount);
    }

    #[test]
    fn extract_gas_transfer_fails_when_last_call_not_transfer() {
        let forwarder = felt!("0x123");

        let calldata = vec![
            felt!("0x1"), // caller
            felt!("0x2"), // nonce
            felt!("0x3"), // execute_after
            felt!("0x4"), // execute_before
            Felt::ONE,    // num_calls = 1
            // Call with wrong selector
            felt!("0x456"),       // to
            selector!("approve"), // wrong selector
            Felt::THREE,          // calldata_len
            forwarder,            // recipient
            felt!("0x789"),       // amount_low
            Felt::ZERO,           // amount_high
        ];

        let parameters = ExecutableDirectInvokeParameters {
            user: Felt::ZERO,
            execute_from_outside_call: Call {
                to: felt!("0x999"),
                selector: selector!("execute_from_outside"),
                calldata,
            },
        };

        let result = parameters.find_gas_token_transfer(forwarder);
        assert!(result.is_err());
    }

    #[test]
    fn extract_gas_transfer_fails_when_recipient_not_forwarder() {
        let forwarder = felt!("0x123");
        let wrong_recipient = felt!("0x456");

        let calldata = vec![
            felt!("0x1"), // caller
            felt!("0x2"), // nonce
            felt!("0x3"), // execute_after
            felt!("0x4"), // execute_before
            Felt::ONE,    // num_calls = 1
            // Transfer to wrong recipient
            felt!("0x789"),        // to
            selector!("transfer"), // selector
            Felt::THREE,           // calldata_len
            wrong_recipient,       // wrong recipient
            felt!("0xAAA"),        // amount_low
            Felt::ZERO,            // amount_high
        ];

        let parameters = ExecutableDirectInvokeParameters {
            user: Felt::ZERO,
            execute_from_outside_call: Call {
                to: felt!("0x999"),
                selector: selector!("execute_from_outside"),
                calldata,
            },
        };

        let result = parameters.find_gas_token_transfer(forwarder);
        assert!(result.is_err());
    }

    #[test]
    fn extract_gas_transfer_fails_when_no_calls() {
        let forwarder = felt!("0x123");

        let calldata = vec![
            felt!("0x1"), // caller
            felt!("0x2"), // nonce
            felt!("0x3"), // execute_after
            felt!("0x4"), // execute_before
            Felt::ZERO,   // num_calls = 0
        ];

        let parameters = ExecutableDirectInvokeParameters {
            user: Felt::ZERO,
            execute_from_outside_call: Call {
                to: felt!("0x999"),
                selector: selector!("execute_from_outside"),
                calldata,
            },
        };

        let result = parameters.find_gas_token_transfer(forwarder);
        assert!(result.is_err());
    }

    #[test]
    fn extract_gas_transfer_fails_when_insufficient_calldata() {
        let forwarder = felt!("0x123");

        // Not enough data
        let calldata = vec![
            felt!("0x1"), // caller
            felt!("0x2"), // nonce
            felt!("0x3"), // execute_after
        ];

        let parameters = ExecutableDirectInvokeParameters {
            user: Felt::ZERO,
            execute_from_outside_call: Call {
                to: felt!("0x999"),
                selector: selector!("execute_from_outside"),
                calldata,
            },
        };

        let result = parameters.find_gas_token_transfer(forwarder);
        assert!(result.is_err());
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
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
            privacy_pool: Felt::ZERO,
            privacy_pool_fee_amount: 0,
        };

        let estimate = transaction.estimate_sponsored_transaction(&client, vec![]).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    async fn execute_invoke_transaction_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);

        let user = StarknetTestEnvironment::ACCOUNT_ARGENT_1;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,

            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: user.address,
                    calls: Calls::new(vec![an_eth_transfer(account.address(), Felt::ONE)]),
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
            privacy_pool: Felt::ZERO,
            privacy_pool_fee_amount: 0,
        };

        let estimate = transaction.estimate_transaction(&client).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
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
                    calls: Calls::new(vec![an_eth_approve(account.address(), Felt::ZERO)]),
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
            privacy_pool: Felt::ZERO,
            privacy_pool_fee_amount: 0,
        };

        let estimate = transaction.estimate_transaction(&client).await.unwrap();
        let result = estimate.execute(&client).await;
        assert!(result.is_ok())
    }
}
