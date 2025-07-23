use std::collections::HashSet;

use jsonrpsee::core::Serialize;
use paymaster_execution::Transaction;
use paymaster_starknet::transaction::Calls;
use serde::Deserialize;
use starknet::core::types::{Call, Felt, TypedData};

use crate::context::Context;
use crate::endpoint::common::{DeploymentParameters, ExecutionParameters};
use crate::endpoint::validation::{check_is_allowed_fee_mode, check_is_supported_token, check_no_blacklisted_call, check_service_is_available};
use crate::endpoint::RequestContext;
use crate::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildTransactionRequest {
    pub transaction: TransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransactionParameters {
    Deploy { deployment: DeploymentParameters },
    Invoke { invoke: InvokeParameters },
    DeployAndInvoke { deployment: DeploymentParameters, invoke: InvokeParameters },
}

impl From<TransactionParameters> for paymaster_execution::TransactionParameters {
    fn from(value: TransactionParameters) -> Self {
        match value {
            TransactionParameters::Deploy { deployment } => Self::Deploy { deployment: deployment.into() },
            TransactionParameters::Invoke { invoke } => Self::Invoke { invoke: invoke.into() },
            TransactionParameters::DeployAndInvoke { deployment, invoke } => Self::DeployAndInvoke {
                deployment: deployment.into(),
                invoke: invoke.into(),
            },
        }
    }
}

impl TransactionParameters {
    pub fn calls(&self) -> &[Call] {
        match self {
            Self::Deploy { .. } => &[],
            Self::Invoke { invoke } => &invoke.calls,
            Self::DeployAndInvoke { invoke, .. } => &invoke.calls,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InvokeParameters {
    pub user_address: Felt,
    pub calls: Vec<Call>,
}

impl From<InvokeParameters> for paymaster_execution::InvokeParameters {
    fn from(value: InvokeParameters) -> Self {
        Self {
            user_address: value.user_address,
            calls: Calls::new(value.calls),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildTransactionResponse {
    Deploy(DeployTransaction),
    Invoke(InvokeTransaction),
    DeployAndInvoke(DeployAndInvokeTransaction),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeployTransaction {
    pub deployment: DeploymentParameters,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

impl From<DeployTransaction> for BuildTransactionResponse {
    fn from(value: DeployTransaction) -> Self {
        Self::Deploy(value)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InvokeTransaction {
    pub typed_data: TypedData,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

impl From<InvokeTransaction> for BuildTransactionResponse {
    fn from(value: InvokeTransaction) -> Self {
        Self::Invoke(value)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeployAndInvokeTransaction {
    pub deployment: DeploymentParameters,
    pub typed_data: TypedData,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

impl From<DeployAndInvokeTransaction> for BuildTransactionResponse {
    fn from(value: DeployAndInvokeTransaction) -> Self {
        Self::DeployAndInvoke(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FeeEstimate {
    pub gas_token_price_in_strk: Felt,
    pub estimated_fee_in_strk: Felt,
    pub estimated_fee_in_gas_token: Felt,
    pub suggested_max_fee_in_strk: Felt,
    pub suggested_max_fee_in_gas_token: Felt,
}

impl From<paymaster_execution::FeeEstimate> for FeeEstimate {
    fn from(value: paymaster_execution::FeeEstimate) -> Self {
        Self {
            gas_token_price_in_strk: value.gas_token_price_in_strk,

            estimated_fee_in_strk: value.estimated_fee_in_strk,
            estimated_fee_in_gas_token: value.estimated_fee_in_gas_token,

            suggested_max_fee_in_strk: value.suggested_max_fee_in_strk,
            suggested_max_fee_in_gas_token: value.suggested_max_fee_in_gas_token,
        }
    }
}

pub async fn build_transaction_endpoint(ctx: &RequestContext<'_>, request: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
    check_service_is_available(ctx).await?;
    check_is_allowed_fee_mode(ctx, &request.parameters).await?;

    // Do preliminary checks
    check_no_blacklisted_call(&request.transaction, &HashSet::new())?;
    check_is_supported_token(&request.parameters, &ctx.configuration.supported_tokens)?;

    match &request.transaction {
        TransactionParameters::Deploy { .. } if request.parameters.fee_mode().is_sponsored() => build_deploy_sponsored(ctx, request).await,
        _ => build_transaction(ctx, request).await,
    }
}

async fn build_deploy_sponsored(ctx: &Context, request: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
    let deployment = match &request.transaction {
        TransactionParameters::Deploy { deployment } => deployment.clone(),
        _ => return Err(Error::InvalidDeploymentData),
    };

    let parameters = request.parameters.clone();

    let transaction = Transaction {
        forwarder: ctx.configuration.forwarder,
        transaction: request.transaction.into(),
        parameters: request.parameters.into(),
    };

    let estimated_transaction = transaction.estimate(&ctx.execution).await?;
    Ok(BuildTransactionResponse::Deploy(DeployTransaction {
        deployment,
        parameters,
        fee: estimated_transaction.fee_estimate.into(),
    }))
}

async fn build_transaction(ctx: &Context, request: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
    let transaction = Transaction {
        forwarder: ctx.configuration.forwarder,
        transaction: request.transaction.into(),
        parameters: request.parameters.into(),
    };

    let estimated_transaction = transaction.estimate(&ctx.execution).await?;
    let versioned_transaction = estimated_transaction.resolve_version(&ctx.execution).await?;

    let typed_data = versioned_transaction.to_execute_from_outside().to_typed_data()?;
    let parameters = versioned_transaction.parameters.into();

    Ok(match versioned_transaction.transaction {
        paymaster_execution::TransactionParameters::Deploy { deployment } => DeployAndInvokeTransaction {
            deployment: deployment.into(),
            typed_data,
            parameters,
            fee: versioned_transaction.fee_estimate.into(),
        }
        .into(),
        paymaster_execution::TransactionParameters::Invoke { .. } => InvokeTransaction {
            typed_data,
            parameters,
            fee: versioned_transaction.fee_estimate.into(),
        }
        .into(),
        paymaster_execution::TransactionParameters::DeployAndInvoke { deployment, .. } => DeployAndInvokeTransaction {
            deployment: deployment.into(),
            typed_data,
            parameters,
            fee: versioned_transaction.fee_estimate.into(),
        }
        .into(),
    })
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::TokenPrice;
    use paymaster_starknet::testing::transaction::an_eth_transfer;
    use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
    use starknet::core::types::Felt;

    use crate::endpoint::build::{build_transaction_endpoint, BuildTransactionRequest, InvokeParameters, TransactionParameters};
    use crate::endpoint::common::{ExecutionParameters, FeeMode};
    use crate::endpoint::RequestContext;
    use crate::testing::TestEnvironment;
    use crate::Error;

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

    #[tokio::test]
    async fn return_error_if_not_available() {
        let test = TestEnvironment::new().await;
        let mut context = test.context().clone();
        context.price = paymaster_prices::Client::mock::<NoPriceOracle>();

        let request_context = RequestContext::empty(&context);

        let request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: Felt::ZERO,
                    calls: vec![],
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                },
                time_bounds: None,
            },
        };

        let result = build_transaction_endpoint(&request_context, request).await;
        assert!(matches!(result, Err(Error::ServiceNotAvailable)))
    }

    #[tokio::test]
    async fn return_error_if_token_not_supported() {
        let test = TestEnvironment::new().await;
        let request_context = RequestContext::empty(&test.context());

        let request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: Felt::ZERO,
                    calls: vec![],
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default { gas_token: Felt::ZERO },
                time_bounds: None,
            },
        };

        let result = build_transaction_endpoint(&request_context, request).await;
        assert!(matches!(result, Err(Error::TokenNotSupported)))
    }

    #[tokio::test]
    async fn build_works_properly() {
        let test = TestEnvironment::new().await;
        let request_context = RequestContext::empty(&test.context());

        let request = BuildTransactionRequest {
            transaction: TransactionParameters::Invoke {
                invoke: InvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    calls: vec![an_eth_transfer(
                        StarknetTestEnvironment::ACCOUNT_2.address,
                        Felt::ONE,
                        &request_context.configuration.starknet.chain_id,
                    )],
                },
            },
            parameters: ExecutionParameters::V1 {
                fee_mode: FeeMode::Default {
                    gas_token: StarknetTestEnvironment::ETH,
                },
                time_bounds: None,
            },
        };

        let result = build_transaction_endpoint(&request_context, request).await;
        assert!(result.is_ok())
    }
}
