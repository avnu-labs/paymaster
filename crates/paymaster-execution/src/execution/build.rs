use paymaster_prices::math::convert_strk_to_token;
use paymaster_starknet::transaction::{Calls, ExecuteFromOutsideMessage, ExecuteFromOutsideParameters, PaymasterVersion, TokenTransfer};
use paymaster_starknet::{ChainID, ContractAddress};
use starknet::core::types::{BroadcastedTransaction, Felt};
use starknet::macros::felt;
use uuid::Uuid;

use crate::diagnostics::DiagnosticClient;
use crate::execution::deploy::DeploymentParameters;
use crate::execution::fee::FeeEstimate;
use crate::execution::ExecutionParameters;
use crate::{Client, Error};

/// Paymaster transaction parameters to be used for building an executable transaction.
#[derive(Debug)]
pub struct Transaction {
    pub forwarder: ContractAddress,
    pub transaction: TransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Debug, Clone)]
pub enum TransactionParameters {
    Deploy { deployment: DeploymentParameters },
    Invoke { invoke: InvokeParameters },
    DeployAndInvoke { deployment: DeploymentParameters, invoke: InvokeParameters },
}

impl TransactionParameters {
    /// Returns the calls corresponding to the transaction to be executed. In the case where
    /// the transaction it's a deploy-only, no calls are induced. Otherwise, the calls correspond
    /// to the invoke portion of the transaction.
    pub fn calls(&self) -> Calls {
        match self {
            TransactionParameters::Deploy { .. } => Calls::new(vec![]),
            TransactionParameters::Invoke { invoke } => invoke.calls.clone(),
            TransactionParameters::DeployAndInvoke { invoke, .. } => invoke.calls.clone(),
        }
    }

    pub fn user_address(&self) -> Felt {
        match self {
            TransactionParameters::Deploy { deployment } => deployment.address,
            TransactionParameters::Invoke { invoke } => invoke.user_address,
            TransactionParameters::DeployAndInvoke { invoke, .. } => invoke.user_address,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InvokeParameters {
    pub user_address: Felt,
    pub calls: Calls,
}

impl Transaction {
    /// Estimate the transaction using the given [`client`]. This function first check is the transaction is
    /// valid which means that the time bounds, if they are set, are valid and that the transaction contains calls
    /// if it is an invoke. Then, it performs an estimate call on Starknet and return both the estimated fee and the
    /// suggested max fee. The former correspond to the actual value returned by Starknet while the latter corresponds
    /// to the fee that should be used to guarantee a valid execution.
    ///
    /// The client's `diagnostic_client` is used to extract and log diagnostic information from simulation errors.
    pub async fn estimate(self, client: &Client) -> Result<EstimatedTransaction, Error> {
        self.check_parameters_valid()?;

        let transactions = self.build_transactions(client).await?;
        let token = client.price.fetch_token(self.parameters.gas_token()).await?;

        let fee_estimate_result = client.starknet.estimate_transactions(&transactions).await;
        let estimated_fee_in_strk: u128 = match fee_estimate_result {
            Ok(estimates) => estimates.into_iter().map(|x| x.overall_fee).sum(),
            Err(e) => {
                // Extract diagnostic information from the failed simulation
                self.report_simulation_error(&client.diagnostic_client, &e).await;
                return Err(e.into());
            },
        };

        // TODO: update this
        let estimated_fee_in_strk = Felt::from(estimated_fee_in_strk);

        let estimated_fee_in_gas_token = convert_strk_to_token(&token, estimated_fee_in_strk, true)?;

        let suggested_max_fee_in_strk = self.compute_max_fee_in_strk(client, estimated_fee_in_strk).await?;
        let suggested_max_fee_in_gas_token = convert_strk_to_token(&token, suggested_max_fee_in_strk, true)?;

        Ok(EstimatedTransaction {
            chain_id: *client.starknet.chain_id(),
            forwarder: self.forwarder,
            transaction: self.transaction,
            parameters: self.parameters,

            fee_estimate: FeeEstimate {
                gas_token_price_in_strk: token.price_in_strk,
                estimated_fee_in_strk,
                estimated_fee_in_gas_token,
                suggested_max_fee_in_strk,
                suggested_max_fee_in_gas_token,
            },
        })
    }

    /// Analyzes a simulation error and logs diagnostic information.
    async fn report_simulation_error(&self, diagnostic_client: &DiagnosticClient, error: &paymaster_starknet::Error) {
        let calls = self.transaction.calls();
        let user_address = self.transaction.user_address();
        diagnostic_client.report(&calls, user_address, error.to_string()).await;
    }

    // Check that the transaction has valid time bounds and that it contains at least one call
    // if it has an invoke.
    fn check_parameters_valid(&self) -> Result<(), Error> {
        if !self.parameters.time_bounds().is_valid() {
            return Err(Error::InvalidTimeBound);
        }

        match &self.transaction {
            TransactionParameters::Deploy { .. } => Ok(()),

            TransactionParameters::Invoke { invoke } if invoke.calls.is_empty() => Err(Error::NoCalls),
            TransactionParameters::Invoke { .. } => Ok(()),

            TransactionParameters::DeployAndInvoke { invoke, .. } if invoke.calls.is_empty() => Err(Error::NoCalls),
            TransactionParameters::DeployAndInvoke { .. } => Ok(()),
        }
    }

    // Compute the max fee estimate that we will suggest to use to guarantee execution. This amount will be approved by the user but should be understood
    // as an upper bound on the real amount that will be paid. A second estimate will be done just before execution and this amount will be the one actually paid
    // so here we just need to ensure that the user has approved enough to compensate for the volatility.
    async fn compute_max_fee_in_strk(&self, client: &Client, base_estimate: Felt) -> Result<Felt, Error> {
        match &self.transaction {
            TransactionParameters::Deploy { .. } => Ok(client.compute_max_fee_in_strk(base_estimate)),
            TransactionParameters::Invoke { invoke } => {
                client
                    .compute_max_fee_with_overhead_in_strk(invoke.user_address, base_estimate)
                    .await
            },
            TransactionParameters::DeployAndInvoke { invoke, .. } => {
                client
                    .compute_max_fee_with_overhead_in_strk(invoke.user_address, base_estimate)
                    .await
            },
        }
    }

    // Convert the transaction into a Starknet transaction type to perform the estimate
    async fn build_transactions(&self, client: &Client) -> Result<Vec<BroadcastedTransaction>, Error> {
        Ok(match &self.transaction {
            // A sponsored transaction only has a deployment without any invoke to pay gas token
            TransactionParameters::Deploy { deployment } if self.parameters.fee_mode().is_sponsored() => {
                let deploy_tx = deployment.build_transaction(client, self.parameters.fee_mode().tip()).await?;

                vec![deploy_tx]
            },
            // A non-sponsored deploy transaction also contains a gas token transfer to pay for the gas
            TransactionParameters::Deploy { deployment } => {
                let deploy_tx = deployment.build_transaction(client, self.parameters.fee_mode().tip()).await?;
                let tip = client.get_tip(self.parameters.tip()).await?;
                let invoke_tx = self.build_invoke(deployment.address, felt!("0x0"), tip);

                vec![deploy_tx, invoke_tx]
            },
            TransactionParameters::Invoke { invoke } => {
                let nonce = client.starknet.fetch_nonce(invoke.user_address).await?;
                let tip = client.get_tip(self.parameters.tip()).await?;
                let invoke_tx = self.build_invoke(invoke.user_address, nonce, tip);

                vec![invoke_tx]
            },
            TransactionParameters::DeployAndInvoke { deployment, invoke } if deployment.address == invoke.user_address => {
                let deploy_tx = deployment.build_transaction(client, self.parameters.fee_mode().tip()).await?;
                let tip = client.get_tip(self.parameters.tip()).await?;
                let invoke_tx = self.build_invoke(deployment.address, felt!("0x0"), tip);

                vec![deploy_tx, invoke_tx]
            },
            TransactionParameters::DeployAndInvoke { deployment, invoke } => {
                let deploy_tx = deployment.build_transaction(client, self.parameters.fee_mode().tip()).await?;

                let nonce = client.starknet.fetch_nonce(invoke.user_address).await?;
                let tip = client.get_tip(self.parameters.tip()).await?;
                let invoke_tx = self.build_invoke(invoke.user_address, nonce, tip);

                vec![deploy_tx, invoke_tx]
            },
        })
    }

    fn build_invoke(&self, sender: Felt, nonce: Felt, tip: u64) -> BroadcastedTransaction {
        let calls = if self.parameters.fee_mode().is_sponsored() {
            self.build_sponsored_calls()
        } else {
            self.build_unsponsored_calls()
        };

        calls.as_transaction(sender, nonce, tip)
    }

    // Build the call for a sponsored transaction which means that we don't include the gas token transfer
    pub fn build_sponsored_calls(&self) -> Calls {
        self.transaction.calls()
    }

    // Build the call for an unsponsored transaction which means that we inject a transfer of the gas token
    // by the user to our forwarder
    pub fn build_unsponsored_calls(&self) -> Calls {
        let mut calls = self.transaction.calls();
        calls.push(TokenTransfer::new(self.parameters.gas_token(), self.forwarder, Felt::ONE).to_call());

        calls
    }
}

/// Paymaster transaction that has been estimated and whose version needs to be resolved to produce an executable
/// transaction
#[derive(Debug)]
pub struct EstimatedTransaction {
    chain_id: ChainID,
    forwarder: ContractAddress,
    pub transaction: TransactionParameters,
    pub parameters: ExecutionParameters,
    pub fee_estimate: FeeEstimate,
}

impl EstimatedTransaction {
    /// Resolve the paymaster version. In the case of a deploy-only or a deploy and invoke where the invoke is executed on the newly deployed
    /// contract, we use the paymaster version associated with the contract. In the case of an invoke on a existing contract, we resolve the
    /// version directly on-chain.
    #[rustfmt::skip]
    pub async fn resolve_version(self, client: &Client) -> Result<VersionedTransaction, Error> {
        let version = match &self.transaction {
            TransactionParameters::Deploy { deployment } =>  {
                client.starknet.resolve_paymaster_version_from_class(deployment.resolve_class_hash()?).await?
            },
            TransactionParameters::Invoke { invoke } => {
                client.starknet.resolve_paymaster_version_from_account(invoke.user_address).await?
            },
            TransactionParameters::DeployAndInvoke { deployment, invoke } if deployment.address == invoke.user_address => {
                client.starknet.resolve_paymaster_version_from_class(deployment.resolve_class_hash()?).await?
            },
            TransactionParameters::DeployAndInvoke { invoke, .. } => {
                client.starknet.resolve_paymaster_version_from_account(invoke.user_address).await?
            },
        };

        Ok(VersionedTransaction {
            chain_id: self.chain_id,
            forwarder: self.forwarder,
            version,
            transaction: self.transaction,
            parameters: self.parameters,
            fee_estimate: self.fee_estimate,
        })
    }
}

/// Paymaster transaction that is fully built and can be converted to an *execute_from_outside* message.
#[derive(Debug)]
pub struct VersionedTransaction {
    chain_id: ChainID,
    forwarder: Felt,
    pub version: PaymasterVersion,
    pub transaction: TransactionParameters,
    pub parameters: ExecutionParameters,
    pub fee_estimate: FeeEstimate,
}

impl VersionedTransaction {
    /// Convert the transaction into an *execute_from_outside* message that needs to be signed by the user.
    pub fn to_execute_from_outside(&self) -> ExecuteFromOutsideMessage {
        let calls = self.build_calls();

        ExecuteFromOutsideMessage::new(
            self.version,
            ExecuteFromOutsideParameters {
                chain_id: self.chain_id,
                caller: self.forwarder,
                nonce: Felt::from(Uuid::new_v4().to_u128_le()),
                calls,
                time_bounds: self.parameters.time_bounds(),
            },
        )
    }

    fn build_calls(&self) -> Calls {
        if self.parameters.fee_mode().is_sponsored() {
            self.build_sponsored_calls()
        } else {
            self.build_unsponsored_calls()
        }
    }

    // Build the call for a sponsored transaction which means that we don't include the gas token transfer
    pub fn build_sponsored_calls(&self) -> Calls {
        self.transaction.calls()
    }

    // Build the call for an unsponsored transaction which means that we inject a transfer of the gas token
    // by the user to our forwarder
    pub fn build_unsponsored_calls(&self) -> Calls {
        let mut calls = self.transaction.calls();
        calls.push(TokenTransfer::new(self.parameters.gas_token(), self.forwarder, self.fee_estimate.suggested_max_fee_in_gas_token).to_call());

        calls
    }
}

#[cfg(test)]
mod tests {
    use paymaster_starknet::transaction::{Calls, PaymasterVersion, TokenTransfer};
    use rand::Rng;
    use starknet::accounts::{Account, AccountFactory};
    use starknet::core::types::Felt;
    use starknet::macros::{felt, selector};

    use crate::execution::build::{InvokeParameters, Transaction, TransactionParameters};
    use crate::execution::deploy::DeploymentParameters;
    use crate::execution::{ExecutionParameters, FeeMode, TipPriority};
    use crate::testing::transaction::an_eth_transfer;
    use crate::testing::{StarknetTestEnvironment, TestEnvironment};

    #[tokio::test]
    async fn build_invoke_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_ARGENT_1);

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: account.address(),
                    calls: Calls::new(vec![an_eth_transfer(account.address(), Felt::from(42), test.starknet.chain_id())]),
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
        let result = estimated_transaction.resolve_version(&client).await.unwrap();

        assert_eq!(result.forwarder, StarknetTestEnvironment::FORWARDER);
        assert_eq!(result.version, PaymasterVersion::V2);

        let calls = result.build_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[0].selector, selector!("transfer"));
        assert_eq!(calls[0].calldata, vec![account.address(), Felt::from(42), Felt::ZERO]);

        assert_eq!(calls[1].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[1].selector, selector!("transfer"));
        assert_eq!(calls[1].calldata, vec![StarknetTestEnvironment::FORWARDER, felt!("0xe38f9fc02c000"), Felt::ZERO]);
    }

    #[tokio::test]
    async fn build_deploy_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);
        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;

        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(&account, &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::ONE))
            .await;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            transaction: TransactionParameters::Deploy {
                deployment: DeploymentParameters {
                    version: 2,
                    address: new_account_address,
                    class_hash: new_account.class_hash(),
                    unique: Felt::ZERO,
                    salt,
                    calldata: new_account.calldata(),
                    sigdata: None,
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
        let result = estimated_transaction.resolve_version(&client).await.unwrap();

        assert_eq!(result.forwarder, StarknetTestEnvironment::FORWARDER);
        assert_eq!(result.version, PaymasterVersion::V2);

        let calls = result.build_calls();
        assert_eq!(calls.len(), 1);

        assert_eq!(calls[0].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[0].selector, selector!("transfer"));
        assert_eq!(calls[0].calldata, vec![StarknetTestEnvironment::FORWARDER, felt!("0x180fe9bc2e4000"), Felt::ZERO]);
    }

    #[tokio::test]
    async fn build_deploy_sponsored_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);
        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;

        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(&account, &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::ONE))
            .await;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            transaction: TransactionParameters::Deploy {
                deployment: DeploymentParameters {
                    version: 2,
                    address: new_account_address,
                    class_hash: new_account.class_hash(),
                    unique: Felt::ZERO,
                    salt,
                    calldata: new_account.calldata(),
                    sigdata: None,
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Sponsored { tip: TipPriority::Normal },
                time_bounds: None,
            },
        };

        let client = test.default_client();

        let estimated_transaction = transaction.estimate(&client).await.unwrap();
        let result = estimated_transaction.resolve_version(&client).await.unwrap();

        assert_eq!(result.forwarder, StarknetTestEnvironment::FORWARDER);
        assert_eq!(result.version, PaymasterVersion::V2);

        let calls = result.build_calls();
        assert_eq!(calls.len(), 0);
    }

    #[tokio::test]
    async fn estimate_deploy_and_invoke_same_contract_works_properly() {
        let test = TestEnvironment::new().await;
        let account = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);
        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;

        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(&account, &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::ONE))
            .await;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            transaction: TransactionParameters::DeployAndInvoke {
                deployment: DeploymentParameters {
                    version: 2,
                    address: new_account_address,
                    class_hash: new_account.class_hash(),
                    unique: Felt::ZERO,
                    salt,
                    calldata: new_account.calldata(),
                    sigdata: None,
                },
                invoke: InvokeParameters {
                    user_address: new_account_address,
                    calls: Calls::new(vec![an_eth_transfer(account.address(), Felt::ZERO, test.starknet.chain_id())]),
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
        let result = estimated_transaction.resolve_version(&client).await.unwrap();

        assert_eq!(result.forwarder, StarknetTestEnvironment::FORWARDER);
        assert_eq!(result.version, PaymasterVersion::V2);

        let calls = result.build_calls();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[0].selector, selector!("transfer"));
        assert_eq!(calls[0].calldata, vec![account.address(), Felt::ZERO, Felt::ZERO]);

        assert_eq!(calls[1].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[1].selector, selector!("transfer"));
        assert_eq!(calls[1].calldata, vec![StarknetTestEnvironment::FORWARDER, felt!("0x1b6253b33b4000"), Felt::ZERO]);
    }

    #[tokio::test]
    async fn estimate_deploy_and_invoke_works_properly() {
        let test = TestEnvironment::new().await;
        let account_1 = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_1);
        let account_2 = test.starknet.initialize_account(&StarknetTestEnvironment::ACCOUNT_ARGENT_1);
        let new_account = test.starknet.initialize_argent_account(Felt::ONE).await;

        let salt = Felt::from(rand::rng().random_range(1..1_000_000_000));
        let new_account_address = new_account.deploy_v3(salt).address();

        test.starknet
            .transfer_token(&account_1, &TokenTransfer::new(StarknetTestEnvironment::ETH, new_account_address, Felt::ONE))
            .await;

        let transaction = Transaction {
            forwarder: StarknetTestEnvironment::FORWARDER,
            transaction: TransactionParameters::DeployAndInvoke {
                deployment: DeploymentParameters {
                    version: 2,
                    address: new_account_address,
                    class_hash: new_account.class_hash(),
                    unique: Felt::ZERO,
                    salt,
                    calldata: new_account.calldata(),
                    sigdata: None,
                },
                invoke: InvokeParameters {
                    user_address: account_2.address(),
                    calls: Calls::new(vec![an_eth_transfer(account_2.address(), Felt::ZERO, test.starknet.chain_id())]),
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
        let result = estimated_transaction.resolve_version(&client).await.unwrap();

        assert_eq!(result.forwarder, StarknetTestEnvironment::FORWARDER);
        assert_eq!(result.version, PaymasterVersion::V2);

        let calls = result.build_calls();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[0].selector, selector!("transfer"));
        assert_eq!(calls[0].calldata, vec![account_2.address(), Felt::ZERO, Felt::ZERO]);

        assert_eq!(calls[1].to, StarknetTestEnvironment::ETH);
        assert_eq!(calls[1].selector, selector!("transfer"));
        assert_eq!(calls[1].calldata, vec![StarknetTestEnvironment::FORWARDER, felt!("0x1b6253b33b4000"), Felt::ZERO]);
    }
}
