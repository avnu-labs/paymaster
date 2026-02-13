use paymaster_rpc::client::Client;
use starknet::core::types::{Call, Felt};
use starknet::signers::Signer;
use crate::*;
use crate::signature::sign_typed_data;

pub struct Unset;
pub struct HasDeploy(DeploymentParameters);
pub struct HasInvoke(Vec<Call>);

/// High-level builder that orchestrates the build, sign, and execute flow.
///
/// Uses the typestate pattern to enforce that a transaction type is set before building.
/// Created via [`PaymasterClient::transaction()`](crate::PaymasterClient::transaction).
///
/// # Example
///
/// ```ignore
/// use starknet::signers::{LocalWallet, SigningKey};
///
/// let client = PaymasterClient::builder("https://sepolia.paymaster.avnu.fi/")
///     .api_key("my-key")
///     .build()?;
/// let wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key));
///
/// // One-step flow
/// client.transaction(account_address)
///     .call(transfer_call)
///     .sponsored()
///     .send(&wallet)
///     .await?;
///
/// // Two-step flow with fee inspection
/// let prepared = client.transaction(account_address)
///     .call(transfer_call)
///     .build()
///     .await?;
///
/// println!("Fee: {:?}", prepared.fee);
/// let result = prepared.send(&wallet).await?;
/// ```
pub struct TransactionBuilder<'a, Deploy, Invoke> {
    client: &'a Client,
    address: Felt,

    deploy: Deploy,
    invoke: Invoke,
    fee: FeeMode,

    time_bounds: Option<TimeBounds>,
}

impl<'a> TransactionBuilder<'a, Unset, Unset> {
    pub fn new(client: &'a Client, address: Felt) -> Self {
        Self {
            client,
            address,

            deploy: Unset,
            invoke: Unset,
            fee: FeeMode::Default { gas_token: STRK_TOKEN, tip: TipPriority::Normal },

            time_bounds: None,
        }
    }
}

impl<'a, Deploy, Invoke> TransactionBuilder<'a, Deploy, Invoke> {
    /// Sets time bounds for transaction execution.
    pub fn time_bounds(mut self, bounds: TimeBounds) -> Self {
        self.time_bounds = Some(bounds);
        self
    }

    /// Sets the calls to include in the invoke transaction.
    pub fn fee_mode(self, fee_mode: FeeMode) -> TransactionBuilder<'a, Deploy, Invoke> {
        TransactionBuilder {
            fee: fee_mode,
            ..self
        }
    }

    pub fn sponsored(self) -> TransactionBuilder<'a, Deploy, Invoke> {
        self.fee_mode(FeeMode::Sponsored { tip: TipPriority::Normal })
    }
}

impl<'a, Invoke> TransactionBuilder<'a, Unset, Invoke> {
    /// Sets deployment parameters for a deploy transaction.
    pub fn deployment(self, deployment: DeploymentParameters) -> TransactionBuilder<'a, HasDeploy, Invoke> {
        TransactionBuilder {
            client: self.client,
            address: self.address,

            deploy: HasDeploy(deployment),
            invoke: self.invoke,
            fee: self.fee,

            time_bounds: self.time_bounds
        }
    }
}

impl<'a, Deploy> TransactionBuilder<'a, Deploy, Unset> {
    pub fn call(self, call: Call) -> TransactionBuilder<'a, Deploy, HasInvoke> {
        self.calls(vec![call])
    }

    /// Sets the calls to include in the invoke transaction.
    pub fn calls(self, calls: Vec<Call>) -> TransactionBuilder<'a, Deploy, HasInvoke> {
        TransactionBuilder {
            client: self.client,
            address: self.address,

            deploy: self.deploy,
            invoke: HasInvoke(calls),
            fee: self.fee,

            time_bounds: self.time_bounds
        }
    }
}

impl<'a> TransactionBuilder<'a, HasDeploy, HasInvoke> {
    /// Builds the transaction and returns a [`PreparedTransaction`] with the fee estimate.
    ///
    /// Use this for a two-step flow where you want to inspect fees before signing.
    pub async fn build(self) -> Result<BuiltTransaction<'a>, Error> {
         let request = BuildTransactionRequest {
            transaction: TransactionParameters::DeployAndInvoke {
                deployment: self.deploy.0,
                invoke: InvokeParameters {
                    user_address: self.address,
                    calls: self.invoke.0,
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: self.fee,
                time_bounds: self.time_bounds,
            },
        };

        let response = self
            .client
            .build_transaction(request)
            .await?;

        Ok(BuiltTransaction {
            client: self.client,
            address: self.address,

            transaction: response,
        })
    }

    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync
    {
        self
            .build()
            .await?
            .sign(signer)
            .await?
            .execute()
            .await
    }
}

impl<'a> TransactionBuilder<'a, HasDeploy, Unset> {
    /// Builds the transaction and returns a [`PreparedTransaction`] with the fee estimate.
    ///
    /// Use this for a two-step flow where you want to inspect fees before signing.
    pub async fn build(self) -> Result<BuiltTransaction<'a>, Error> {
        if !matches!(self.fee, FeeMode::Sponsored { .. }) {
            return Err(Error::Configuration("deploy-only only supported in sponsored mode".to_string()))
        }

        let request = BuildTransactionRequest {
            transaction: TransactionParameters::Deploy {
                deployment: self.deploy.0,
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: self.fee,
                time_bounds: self.time_bounds,
            },
        };

        let response = self
            .client
            .build_transaction(request)
            .await?;

        Ok(BuiltTransaction {
            client: self.client,
            address: self.address,

            transaction: response,
        })
    }

    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync
    {
        self
            .build()
            .await?
            .sign(signer)
            .await?
            .execute()
            .await
    }
}

impl<'a> TransactionBuilder<'a, Unset, HasInvoke> {
    /// Builds the transaction and returns a [`PreparedTransaction`] with the fee estimate.
    ///
    /// Use this for a two-step flow where you want to inspect fees before signing.
    pub async fn build(self) -> Result<BuiltTransaction<'a>, Error> {
        let request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: self.address,
                    calls: self.invoke.0,
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: self.fee,
                time_bounds: self.time_bounds,
            },
        };

        let response = self
            .client
            .build_transaction(request)
            .await?;

        Ok(BuiltTransaction {
            client: self.client,
            address: self.address,

            transaction: response,
        })
    }

    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync
    {
        self
            .build()
            .await?
            .sign(signer)
            .await?
            .execute()
            .await
    }
}

/// A transaction that has been built and is ready to be signed and sent.
///
/// Contains the fee estimate from the build step, allowing inspection before signing.
pub struct BuiltTransaction<'a> {
    client: &'a Client,
    address: Felt,

    transaction: BuildTransactionResponse,
}

impl<'a> BuiltTransaction<'a> {
    pub fn fee_estimate(&self) -> FeeEstimate {
        match &self.transaction {
            BuildTransactionResponse::Invoke(tx) => tx.fee.clone(),
            BuildTransactionResponse::DeployAndInvoke(tx) => tx.fee.clone(),
            BuildTransactionResponse::Deploy(tx) => tx.fee.clone(),
        }
    }

    pub fn execution_parameters(&self) -> ExecutionParameters {
        match &self.transaction {
            BuildTransactionResponse::Invoke(tx) => tx.parameters.clone(),
            BuildTransactionResponse::DeployAndInvoke(tx) => tx.parameters.clone(),
            BuildTransactionResponse::Deploy(tx) => tx.parameters.clone(),
        }
    }

    pub async fn sign<S>(self, signer: &'a S) -> Result<ExecutableTransaction<'a>, Error>
    where
        S: Signer + Send + Sync
    {
        Ok(ExecutableTransaction {
            client: self.client,

            transaction: match &self.transaction {
                BuildTransactionResponse::Invoke(tx) => self.build_invoke(signer, tx).await?,
                BuildTransactionResponse::DeployAndInvoke(tx) => self.build_deploy_and_invoke(signer, tx).await?,
                BuildTransactionResponse::Deploy(tx) => self.build_deploy(tx)?,
            },
            parameters: self.execution_parameters()
        })
    }

    fn build_deploy(&self, tx: &DeployTransaction) -> Result<ExecutableTransactionParameters, Error> {
        Ok(ExecutableTransactionParameters::Deploy {
            deployment: tx.deployment.clone(),
        })
    }

    async fn build_invoke<S>(&self, signer: &S, tx: &InvokeTransaction) -> Result<ExecutableTransactionParameters, Error>
    where
        S: Signer + Send + Sync
    {
        Ok(ExecutableTransactionParameters::Invoke {
            invoke: ExecutableInvokeParameters {
                user_address: self.address,
                typed_data: tx.typed_data.clone(),
                signature: sign_typed_data(&tx.typed_data, self.address, signer).await?,
            },
        })
    }

    async fn build_deploy_and_invoke<S>(&self, signer: &S, tx: &DeployAndInvokeTransaction) -> Result<ExecutableTransactionParameters, Error>
    where
        S: Signer + Send + Sync
    {
        Ok(ExecutableTransactionParameters::DeployAndInvoke {
            deployment: tx.deployment.clone(),
            invoke: ExecutableInvokeParameters {
                user_address: self.address,
                typed_data: tx.typed_data.clone(),
                signature: sign_typed_data(&tx.typed_data, self.address, signer).await?,
            },
        })
    }

    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync
    {
        self
            .sign(signer)
            .await?
            .execute()
            .await
    }
}

pub struct ExecutableTransaction<'a> {
    client: &'a Client,

    transaction: ExecutableTransactionParameters,
    parameters: ExecutionParameters
}

impl<'a> ExecutableTransaction<'a> {
    pub async fn execute(self) -> Result<ExecuteResponse, Error> {
        let request = ExecuteRequest {
            transaction: self.transaction,
            parameters: self.parameters,
        };

        let response = self
            .client
            .execute_transaction(request)
            .await?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use starknet::signers::{LocalWallet, SigningKey};
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use super::*;
    use crate::PaymasterClient;

    struct JsonRpcOk(serde_json::Value);

    impl Respond for JsonRpcOk {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let id = body.get("id").cloned().unwrap_or(serde_json::json!(0));
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": self.0,
                "id": id
            }))
        }
    }

    fn test_wallet() -> LocalWallet {
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(Felt::from_hex_unchecked("0x5678")))
    }

    fn typed_data_json() -> serde_json::Value {
        serde_json::json!({
            "types": {
                "StarknetDomain": [
                    {"name": "name", "type": "shortstring"},
                    {"name": "version", "type": "shortstring"},
                    {"name": "chainId", "type": "shortstring"},
                    {"name": "revision", "type": "shortstring"}
                ],
                "OutsideExecution": [
                    {"name": "Caller", "type": "ContractAddress"},
                    {"name": "Nonce", "type": "felt"},
                    {"name": "Execute After", "type": "u128"},
                    {"name": "Execute Before", "type": "u128"},
                    {"name": "Calls", "type": "Call*"}
                ],
                "Call": [
                    {"name": "To", "type": "ContractAddress"},
                    {"name": "Selector", "type": "selector"},
                    {"name": "Calldata", "type": "felt*"}
                ]
            },
            "primaryType": "OutsideExecution",
            "domain": {
                "name": "Account.execute_from_outside",
                "version": "2",
                "chainId": "0x534e5f5345504f4c4941",
                "revision": "1"
            },
            "message": {
                "Caller": "0x414e595f43414c4c4552",
                "Nonce": "0x1",
                "Execute After": "0x0",
                "Execute Before": "0xffffffffffffffff",
                "Calls": []
            }
        })
    }

    fn invoke_build_result() -> serde_json::Value {
        serde_json::json!({
            "type": "invoke",
            "typed_data": typed_data_json(),
            "parameters": {
                "version": "0x1",
                "fee_mode": {"mode": "sponsored", "tip": "normal"},
                "time_bounds": null
            },
            "fee": {
                "gas_token_price_in_strk": "0x1",
                "estimated_fee_in_strk": "0x100",
                "estimated_fee_in_gas_token": "0x100",
                "suggested_max_fee_in_strk": "0x200",
                "suggested_max_fee_in_gas_token": "0x200"
            }
        })
    }

    fn deploy_build_result() -> serde_json::Value {
        serde_json::json!({
            "type": "deploy",
            "deployment": {
                "address": "0x1",
                "class_hash": "0x2",
                "salt": "0x3",
                "calldata": [],
                "sigdata": null,
                "version": 1
            },
            "parameters": {
                "version": "0x1",
                "fee_mode": {"mode": "sponsored", "tip": "normal"},
                "time_bounds": null
            },
            "fee": {
                "gas_token_price_in_strk": "0x1",
                "estimated_fee_in_strk": "0x50",
                "estimated_fee_in_gas_token": "0x50",
                "suggested_max_fee_in_strk": "0xa0",
                "suggested_max_fee_in_gas_token": "0xa0"
            }
        })
    }

    fn execute_result() -> serde_json::Value {
        serde_json::json!({
            "transaction_hash": "0xdeadbeef",
            "tracking_id": "0x0"
        })
    }

    #[tokio::test]
    async fn should_default_gas_token_to_strk() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "paymaster_buildTransaction",
                "params": [{
                    "parameters": {
                        "fee_mode": {
                            "mode": "default",
                            "gas_token": "0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d"
                        }
                    }
                }]
            })))
            .respond_with(JsonRpcOk(invoke_build_result()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_executeTransaction"})))
            .respond_with(JsonRpcOk(execute_result()))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(&server.uri());
        client
            .transaction(Felt::from_hex_unchecked("0x1234"))
            .calls(vec![])
            .send(&test_wallet())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn should_reject_deploy_only_when_not_sponsored() {
        let client = PaymasterClient::new("http://localhost:1234");
        let result = client
            .transaction(Felt::ONE)
            .deployment(DeploymentParameters {
                address: Felt::ONE,
                class_hash: Felt::TWO,
                salt: Felt::THREE,
                calldata: vec![],
                sigdata: None,
                version: 1,
            })
            .fee_mode(FeeMode::Sponsored { tip: TipPriority::Normal })
            .send(&test_wallet())
            .await;

        assert!(matches!(result, Err(Error::Configuration(ref msg)) if msg.contains("sponsored")));
    }

    #[tokio::test]
    async fn should_execute_deploy_only_when_sponsored() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_buildTransaction"})))
            .respond_with(JsonRpcOk(deploy_build_result()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_executeTransaction"})))
            .respond_with(JsonRpcOk(execute_result()))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(&server.uri());
        let result = client
            .transaction(Felt::ONE)
            .deployment(DeploymentParameters {
                address: Felt::ONE,
                class_hash: Felt::TWO,
                salt: Felt::THREE,
                calldata: vec![],
                sigdata: None,
                version: 1,
            })
            .sponsored()
            .send(&test_wallet())
            .await
            .unwrap();

        assert_eq!(result.transaction_hash, Felt::from_hex_unchecked("0xdeadbeef"));
    }

    #[tokio::test]
    async fn should_return_fee_estimate_when_build() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_buildTransaction"})))
            .respond_with(JsonRpcOk(invoke_build_result()))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(&server.uri());
        let prepared = client
            .transaction(Felt::from_hex_unchecked("0x1234"))
            .calls(vec![])
            .sponsored()
            .build()
            .await
            .unwrap();

        assert_eq!(prepared.fee_estimate().estimated_fee_in_strk, Felt::from_hex_unchecked("0x100"));
        assert_eq!(prepared.fee_estimate().suggested_max_fee_in_strk, Felt::from_hex_unchecked("0x200"));
    }

    #[tokio::test]
    async fn should_execute_after_build_when_two_step_flow() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_buildTransaction"})))
            .respond_with(JsonRpcOk(invoke_build_result()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_executeTransaction"})))
            .respond_with(JsonRpcOk(execute_result()))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(&server.uri());
        let prepared = client
            .transaction(Felt::from_hex_unchecked("0x1234"))
            .calls(vec![])
            .sponsored()
            .build()
            .await
            .unwrap();

        let result = prepared
            .send(&test_wallet())
            .await
            .unwrap();

        assert_eq!(result.transaction_hash, Felt::from_hex_unchecked("0xdeadbeef"));
    }

    #[tokio::test]
    async fn should_complete_full_invoke_flow() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_buildTransaction"})))
            .respond_with(JsonRpcOk(invoke_build_result()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_executeTransaction"})))
            .respond_with(JsonRpcOk(execute_result()))
            .expect(1)
            .mount(&server)
            .await;

        let wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(Felt::from_hex_unchecked("0x5678")));
        let client = PaymasterClient::new(&server.uri());
        let result = client
            .transaction(Felt::from_hex_unchecked("0x1234"))
            .calls(vec![])
            .sponsored()
            .send(&wallet)
            .await
            .unwrap();

        assert_eq!(result.transaction_hash, Felt::from_hex_unchecked("0xdeadbeef"));
    }
}
