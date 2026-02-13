use paymaster_rpc::client::Client;
use starknet::core::types::{Call, Felt, TypedData};
use starknet::signers::Signer;

use crate::types::*;
use crate::Error;

/// STRK token address on Starknet.
pub const STRK_TOKEN: Felt = Felt::from_hex_unchecked("0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

/// High-level builder that orchestrates the build, sign, and execute flow.
///
/// # Example
///
/// ```ignore
/// use starknet::signers::{LocalWallet, SigningKey};
///
/// let wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key));
///
/// // One-step flow
/// let result = TransactionBuilder::new(&client)
///     .call(transfer_call)
///     .address(account_address)
///     .sponsored()
///     .send(&wallet)
///     .await?;
///
/// // Two-step flow with fee inspection
/// let prepared = TransactionBuilder::new(&client)
///     .call(transfer_call)
///     .address(account_address)
///     .build()
///     .await?;
///
/// println!("Fee: {:?}", prepared.fee);
/// let result = prepared.send(&wallet).await?;
/// ```
pub struct TransactionBuilder<'a> {
    client: &'a Client,
    calls: Option<Vec<Call>>,
    address: Option<Felt>,
    deployment: Option<DeploymentParameters>,
    gas_token: Option<Felt>,
    sponsored: bool,
    tip: TipPriority,
    time_bounds: Option<TimeBounds>,
}

impl<'a> TransactionBuilder<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self {
            client,
            calls: None,
            address: None,
            deployment: None,
            gas_token: None,
            sponsored: false,
            tip: TipPriority::Normal,
            time_bounds: None,
        }
    }

    /// Sets the calls to include in the invoke transaction.
    pub fn calls(mut self, calls: Vec<Call>) -> Self {
        self.calls = Some(calls);
        self
    }

    /// Sets a single call for the invoke transaction.
    /// Convenience wrapper around [`calls()`](TransactionBuilder::calls).
    pub fn call(self, call: Call) -> Self {
        self.calls(vec![call])
    }

    /// Sets the address of the account that will sign and execute the transaction.
    pub fn address(mut self, address: Felt) -> Self {
        self.address = Some(address);
        self
    }

    /// Sets deployment parameters for a deploy or deploy-and-invoke transaction.
    pub fn deployment(mut self, deployment: DeploymentParameters) -> Self {
        self.deployment = Some(deployment);
        self
    }

    /// Sets the gas token address. Defaults to STRK if not specified.
    pub fn gas_token(mut self, token: Felt) -> Self {
        self.gas_token = Some(token);
        self
    }

    /// Enables sponsored fee mode.
    pub fn sponsored(mut self) -> Self {
        self.sponsored = true;
        self
    }

    /// Sets the tip priority (default: Normal).
    pub fn tip(mut self, tip: TipPriority) -> Self {
        self.tip = tip;
        self
    }

    /// Sets time bounds for transaction execution.
    pub fn time_bounds(mut self, bounds: TimeBounds) -> Self {
        self.time_bounds = Some(bounds);
        self
    }

    /// Builds the transaction and returns a [`PreparedTransaction`] with the fee estimate.
    ///
    /// Use this for a two-step flow where you want to inspect fees before signing.
    pub async fn build(self) -> Result<PreparedTransaction<'a>, Error> {
        let address = self.address.ok_or_else(|| Error::Configuration("address is required".into()))?;

        let transaction = match (&self.deployment, &self.calls) {
            (Some(deployment), Some(calls)) => TransactionParameters::DeployAndInvoke {
                deployment: deployment.clone(),
                invoke: InvokeParameters {
                    user_address: address,
                    calls: calls.clone(),
                },
            },
            (Some(deployment), None) => TransactionParameters::Deploy { deployment: deployment.clone() },
            (None, Some(calls)) => TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: address,
                    calls: calls.clone(),
                },
            },
            (None, None) => {
                return Err(Error::Configuration("either calls or deployment is required".into()));
            },
        };

        if matches!(transaction, TransactionParameters::Deploy { .. }) && !self.sponsored {
            return Err(Error::Configuration("deploy-only transactions must be sponsored (use .sponsored())".into()));
        }

        let fee_mode = if self.sponsored {
            FeeMode::Sponsored { tip: self.tip }
        } else {
            let gas_token = self.gas_token.unwrap_or(STRK_TOKEN);
            FeeMode::Default { gas_token, tip: self.tip }
        };

        let parameters = ExecutionParameters::V1 {
            fee_mode,
            time_bounds: self.time_bounds,
        };

        let build_req = BuildTransactionRequest {
            transaction,
            parameters: parameters.clone(),
        };
        let build_response = self.client.build_transaction(build_req).await?;

        let fee = match &build_response {
            BuildTransactionResponse::Invoke(tx) => tx.fee.clone(),
            BuildTransactionResponse::DeployAndInvoke(tx) => tx.fee.clone(),
            BuildTransactionResponse::Deploy(tx) => tx.fee.clone(),
        };

        Ok(PreparedTransaction {
            client: self.client,
            build_response,
            address,
            parameters,
            fee,
        })
    }

    /// Executes the full build, sign, and execute flow.
    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync,
    {
        self.build().await?.send(signer).await
    }
}

/// A transaction that has been built and is ready to be signed and sent.
///
/// Contains the fee estimate from the build step, allowing inspection before signing.
pub struct PreparedTransaction<'a> {
    client: &'a Client,
    build_response: BuildTransactionResponse,
    address: Felt,
    parameters: ExecutionParameters,
    /// The estimated fee for this transaction.
    pub fee: FeeEstimate,
}

impl<'a> PreparedTransaction<'a> {
    /// Signs and sends the prepared transaction.
    pub async fn send<S>(self, signer: &S) -> Result<ExecuteResponse, Error>
    where
        S: Signer + Send + Sync,
    {
        let exec_transaction = match self.build_response {
            BuildTransactionResponse::Invoke(ref tx) => {
                let signature = sign_typed_data(&tx.typed_data, self.address, signer).await?;
                ExecutableTransactionParameters::Invoke {
                    invoke: ExecutableInvokeParameters {
                        user_address: self.address,
                        typed_data: tx.typed_data.clone(),
                        signature,
                    },
                }
            },
            BuildTransactionResponse::DeployAndInvoke(ref tx) => {
                let signature = sign_typed_data(&tx.typed_data, self.address, signer).await?;
                ExecutableTransactionParameters::DeployAndInvoke {
                    deployment: tx.deployment.clone(),
                    invoke: ExecutableInvokeParameters {
                        user_address: self.address,
                        typed_data: tx.typed_data.clone(),
                        signature,
                    },
                }
            },
            BuildTransactionResponse::Deploy(ref tx) => ExecutableTransactionParameters::Deploy {
                deployment: tx.deployment.clone(),
            },
        };

        let exec_req = ExecuteRequest {
            transaction: exec_transaction,
            parameters: self.parameters,
        };

        Ok(self.client.execute_transaction(exec_req).await?)
    }
}

async fn sign_typed_data<S>(typed_data: &TypedData, address: Felt, signer: &S) -> Result<Vec<Felt>, Error>
where
    S: Signer + Send + Sync,
{
    let message_hash = typed_data
        .message_hash(address)
        .map_err(|e| Error::Signing(format!("failed to compute message hash: {e}")))?;
    let sig = signer
        .sign_hash(&message_hash)
        .await
        .map_err(|e| Error::Signing(e.to_string()))?;
    Ok(vec![sig.r, sig.s])
}

#[cfg(test)]
mod tests {
    use starknet::signers::{LocalWallet, SigningKey};
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use super::*;

    type PaymasterClient = paymaster_rpc::client::Client;

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
    async fn should_error_without_address() {
        let client = PaymasterClient::new("http://localhost:1234");
        let result = TransactionBuilder::new(&client).calls(vec![]).send(&test_wallet()).await;
        assert!(matches!(result, Err(Error::Configuration(_))));
    }

    #[tokio::test]
    async fn should_error_without_calls_or_deployment() {
        let client = PaymasterClient::new("http://localhost:1234");
        let result = TransactionBuilder::new(&client).address(Felt::ONE).send(&test_wallet()).await;
        assert!(matches!(result, Err(Error::Configuration(_))));
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
        TransactionBuilder::new(&client)
            .calls(vec![])
            .address(Felt::from_hex_unchecked("0x1234"))
            .send(&test_wallet())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn should_reject_deploy_only_when_not_sponsored() {
        let client = PaymasterClient::new("http://localhost:1234");
        let result = TransactionBuilder::new(&client)
            .deployment(DeploymentParameters {
                address: Felt::ONE,
                class_hash: Felt::TWO,
                salt: Felt::THREE,
                calldata: vec![],
                sigdata: None,
                version: 1,
            })
            .address(Felt::ONE)
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
        let result = TransactionBuilder::new(&client)
            .deployment(DeploymentParameters {
                address: Felt::ONE,
                class_hash: Felt::TWO,
                salt: Felt::THREE,
                calldata: vec![],
                sigdata: None,
                version: 1,
            })
            .address(Felt::ONE)
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
        let prepared = TransactionBuilder::new(&client)
            .calls(vec![])
            .address(Felt::from_hex_unchecked("0x1234"))
            .sponsored()
            .build()
            .await
            .unwrap();

        assert_eq!(prepared.fee.estimated_fee_in_strk, Felt::from_hex_unchecked("0x100"));
        assert_eq!(prepared.fee.suggested_max_fee_in_strk, Felt::from_hex_unchecked("0x200"));
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
        let prepared = TransactionBuilder::new(&client)
            .calls(vec![])
            .address(Felt::from_hex_unchecked("0x1234"))
            .sponsored()
            .build()
            .await
            .unwrap();

        let result = prepared.send(&test_wallet()).await.unwrap();
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
        let result = TransactionBuilder::new(&client)
            .calls(vec![])
            .address(Felt::from_hex_unchecked("0x1234"))
            .sponsored()
            .send(&wallet)
            .await
            .unwrap();

        assert_eq!(result.transaction_hash, Felt::from_hex_unchecked("0xdeadbeef"));
    }
}
