use paymaster_common::{measure_duration, metric};
use paymaster_execution::ExecutableTransaction;
use paymaster_starknet::transaction::{Calls, PrivateProofData};
use paymaster_starknet::Signature;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{Call, Felt, TypedData};
use starknet::macros::selector;

use crate::endpoint::common::{DeploymentParameters, ExecutionParameters};
use crate::endpoint::validation::check_service_is_available;
use crate::endpoint::RequestContext;
use crate::Error;

#[derive(Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub transaction: ExecutableTransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    PrivateInvoke {
        private_invoke: ExecutablePrivateInvokeParameters,
    },
}

impl TryFrom<ExecutableTransactionParameters> for paymaster_execution::ExecutableTransactionParameters {
    type Error = Error;

    fn try_from(value: ExecutableTransactionParameters) -> Result<Self, Self::Error> {
        Ok(match value {
            ExecutableTransactionParameters::Deploy { deployment } => Self::Deploy { deployment: deployment.into() },
            ExecutableTransactionParameters::Invoke { invoke } => Self::Invoke { invoke: invoke.try_into()? },
            ExecutableTransactionParameters::DeployAndInvoke { deployment, invoke } => Self::DeployAndInvoke {
                deployment: deployment.into(),
                invoke: invoke.try_into()?,
            },
            ExecutableTransactionParameters::PrivateInvoke { private_invoke } => {
                if private_invoke.proof.is_empty() || private_invoke.proof_facts.is_empty() {
                    return Err(Error::PrivacyProofMissing);
                }
                Self::PrivateInvoke {
                    private_invoke: paymaster_execution::ExecutablePrivateInvokeParameters {
                        pool_address: private_invoke.pool_address,
                        calldata: private_invoke.calldata,
                        proof_data: PrivateProofData {
                            proof: private_invoke.proof,
                            proof_facts: private_invoke.proof_facts,
                        },
                    },
                }
            },
        })
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct ExecutableInvokeParameters {
    #[serde_as(as = "UfeHex")]
    pub user_address: Felt,

    pub typed_data: TypedData,

    #[serde_as(as = "Vec<UfeHex>")]
    pub signature: Signature,
}

impl TryFrom<ExecutableInvokeParameters> for paymaster_execution::ExecutableInvokeParameters {
    type Error = Error;

    fn try_from(value: ExecutableInvokeParameters) -> Result<Self, Self::Error> {
        let result = Self::new(value.user_address, value.typed_data, value.signature)?;

        Ok(result)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct ExecutablePrivateInvokeParameters {
    #[serde_as(as = "UfeHex")]
    pub pool_address: Felt,

    #[serde_as(as = "Vec<UfeHex>")]
    pub calldata: Vec<Felt>,

    pub proof: Vec<u64>,

    #[serde_as(as = "Vec<UfeHex>")]
    pub proof_facts: Vec<Felt>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteResponse {
    #[serde_as(as = "UfeHex")]
    pub transaction_hash: Felt,

    #[serde_as(as = "UfeHex")]
    pub tracking_id: Felt,
}

pub async fn execute_endpoint(ctx: &RequestContext<'_>, request: ExecuteRequest) -> Result<ExecuteResponse, Error> {
    check_service_is_available(ctx).await?;

    let execution_params: paymaster_execution::ExecutionParameters = request.parameters.into();
    let transaction_params: paymaster_execution::ExecutableTransactionParameters = request.transaction.try_into()?;
    ctx.transaction_filter.filter(&transaction_params)?;

    match transaction_params {
        paymaster_execution::ExecutableTransactionParameters::PrivateInvoke { private_invoke } => execute_private_invoke(ctx, private_invoke, execution_params).await,
        _ => {
            let transaction = ExecutableTransaction {
                forwarder: ctx.configuration.forwarder,
                gas_tank_address: ctx.configuration.gas_tank.address,
                parameters: execution_params,
                transaction: transaction_params,
            };

            let estimated_transaction = if transaction.parameters.fee_mode().is_sponsored() {
                let authenticated_api_key = ctx.validate_api_key().await?;
                transaction
                    .estimate_sponsored_transaction(&ctx.execution, authenticated_api_key.sponsor_metadata)
                    .await?
            } else {
                transaction.estimate_transaction(&ctx.execution).await?
            };

            let result = estimated_transaction.execute(&ctx.execution).await?;

            Ok(ExecuteResponse {
                transaction_hash: result.transaction_hash,
                tracking_id: Felt::ZERO,
            })
        },
    }
}

async fn execute_private_invoke(
    ctx: &RequestContext<'_>,
    params: paymaster_execution::ExecutablePrivateInvokeParameters,
    execution_params: paymaster_execution::ExecutionParameters,
) -> Result<ExecuteResponse, Error> {
    let privacy_config = ctx.configuration.privacy.as_ref().ok_or(Error::PrivacyPoolNotConfigured)?;
    let pool_config = privacy_config
        .pools
        .get(&params.pool_address)
        .ok_or(Error::PrivacyPoolNotConfigured)?;

    let pool_fee = ctx
        .execution
        .privacy_pool_client()
        .ok_or(Error::PrivacyPoolNotConfigured)?
        .get_fee_amount(params.pool_address)
        .await
        .map_err(|_| Error::PrivacyPoolNotConfigured)?;

    // Build calls: [approve(STRK, pool, pool_fee), pool.apply_actions(calldata)]
    let approve_call = Call {
        to: pool_config.strk_token_address,
        selector: selector!("approve"),
        calldata: vec![params.pool_address, Felt::from(pool_fee), Felt::ZERO],
    };
    let apply_actions_call = Call {
        to: params.pool_address,
        selector: selector!("apply_actions"),
        calldata: params.calldata,
    };
    let calls = Calls::new(vec![approve_call, apply_actions_call]);

    let pool_addr = params.pool_address.to_fixed_hex_string();

    let estimated_calls = ctx
        .execution
        .estimate_with_proof(&calls, execution_params.tip(), &params.proof_data)
        .await?;
    let (result, duration) = measure_duration!(ctx.execution.execute(&estimated_calls, Some(&params.proof_data)).await);

    metric!(counter[privacy_execution_request] = 1, pool_address = pool_addr);
    metric!(
        histogram[privacy_execution_request_duration_milliseconds] = duration.as_millis(),
        pool_address = pool_addr
    );

    match result {
        Ok(result) => Ok(ExecuteResponse {
            transaction_hash: result.transaction_hash,
            tracking_id: Felt::ZERO,
        }),
        Err(e) => {
            metric!(counter[privacy_execution_request_error] = 1, pool_address = pool_addr, error = e.to_string());
            Err(e.into())
        },
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::endpoint::build::{build_transaction_endpoint, BuildTransactionRequest, BuildTransactionResponse, InvokeParameters, TransactionParameters};
    use crate::endpoint::common::{ExecutionParameters, FeeMode, TipPriority};
    use crate::endpoint::execute::{execute_endpoint, ExecutableInvokeParameters, ExecutableTransactionParameters, ExecuteRequest};
    use crate::endpoint::RequestContext;
    use crate::testing::TestEnvironment;
    use crate::{Error, InvokeTransaction};
    use async_trait::async_trait;
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::TokenPrice;
    use paymaster_starknet::testing::transaction::an_eth_transfer;
    use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
    use starknet::core::types::Felt;
    use starknet::signers::SigningKey;

    #[derive(Debug, Clone)]
    struct NoPriceOracle;

    #[async_trait]
    impl MockPriceOracle for NoPriceOracle {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn fetch_token(&self, _: Felt) -> Result<TokenPrice, paymaster_prices::Error> {
            Ok(TokenPrice {
                address: Felt::ZERO,
                price_in_strk: Felt::ZERO,
                decimals: 18,
            })
        }
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    async fn return_error_if_not_available() {
        let test = TestEnvironment::new().await;

        let mut context = test.context().clone();

        let build_request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    calls: vec![an_eth_transfer(StarknetTestEnvironment::ACCOUNT_2.address, Felt::ONE)],
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

        let build_response = build_transaction_endpoint(&RequestContext::empty(&context), build_request)
            .await
            .unwrap();
        let BuildTransactionResponse::Invoke(InvokeTransaction { typed_data, .. }) = build_response else {
            unreachable!()
        };

        // set no token available
        context.price = paymaster_prices::Client::mock::<NoPriceOracle>();

        let request = ExecuteRequest {
            transaction: ExecutableTransactionParameters::Invoke {
                invoke: ExecutableInvokeParameters {
                    user_address: Felt::ZERO,
                    typed_data,
                    signature: vec![Felt::ZERO, Felt::ZERO],
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

        let result = execute_endpoint(&RequestContext::empty(&context), request).await;
        assert!(matches!(result, Err(Error::ServiceNotAvailable)))
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    async fn execute_works_properly() {
        let test = TestEnvironment::new().await;
        let request_context = RequestContext::empty(&test.context());

        let build_request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    calls: vec![an_eth_transfer(StarknetTestEnvironment::ACCOUNT_2.address, Felt::ONE)],
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

        let build_response = build_transaction_endpoint(&request_context, build_request).await.unwrap();
        let BuildTransactionResponse::Invoke(InvokeTransaction { typed_data, .. }) = build_response else {
            unreachable!()
        };

        let message_hash = typed_data
            .message_hash(StarknetTestEnvironment::ACCOUNT_ARGENT_1.address)
            .unwrap();
        let signature = SigningKey::from_secret_scalar(StarknetTestEnvironment::ACCOUNT_ARGENT_1.private_key)
            .sign(&message_hash)
            .unwrap();

        let request = ExecuteRequest {
            transaction: ExecutableTransactionParameters::Invoke {
                invoke: ExecutableInvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    typed_data,
                    signature: vec![signature.r, signature.s],
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

        let result = execute_endpoint(&request_context, request).await;
        assert!(result.is_ok())
    }
}
