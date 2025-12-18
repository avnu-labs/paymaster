use paymaster_execution::ExecutableTransaction;
use paymaster_starknet::Signature;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{Felt, TypedData};

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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteResponse {
    #[serde_as(as = "UfeHex")]
    pub transaction_hash: Felt,

    #[serde_as(as = "UfeHex")]
    pub tracking_id: Felt,
}

pub async fn execute_endpoint(ctx: &RequestContext<'_>, request: ExecuteRequest) -> Result<ExecuteResponse, Error> {
    check_service_is_available(ctx).await?;

    let forwarder = ctx.configuration.forwarder;
    let gas_tank_address = ctx.configuration.gas_tank.address;

    let transaction = ExecutableTransaction {
        forwarder,
        gas_tank_address,
        parameters: request.parameters.into(),
        transaction: request.transaction.try_into()?,
    };

    ctx.transaction_filter.filter(&transaction.transaction)?;

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
