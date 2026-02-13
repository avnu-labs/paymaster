use paymaster_execution::ExecutableTransaction;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{Call, Felt};

use crate::endpoint::common::ExecutionParameters;
use crate::endpoint::validation::check_service_is_available;
use crate::endpoint::RequestContext;
use crate::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteDirectRequest {
    pub transaction: ExecuteDirectTransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecuteDirectTransactionParameters {
    Invoke { invoke: DirectInvokeParameters },
}

impl From<ExecuteDirectTransactionParameters> for paymaster_execution::ExecutableTransactionParameters {
    fn from(value: ExecuteDirectTransactionParameters) -> Self {
        match value {
            ExecuteDirectTransactionParameters::Invoke { invoke } => Self::DirectInvoke { invoke: invoke.into() },
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectInvokeParameters {
    #[serde_as(as = "UfeHex")]
    pub user_address: Felt,

    pub execute_from_outside_call: Call,
}

impl From<DirectInvokeParameters> for paymaster_execution::ExecutableDirectInvokeParameters {
    fn from(value: DirectInvokeParameters) -> Self {
        Self {
            user: value.user_address,
            execute_from_outside_call: value.execute_from_outside_call,
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteDirectResponse {
    #[serde_as(as = "UfeHex")]
    pub transaction_hash: Felt,

    #[serde_as(as = "UfeHex")]
    pub tracking_id: Felt,
}

pub async fn execute_direct_endpoint(ctx: &RequestContext<'_>, request: ExecuteDirectRequest) -> Result<ExecuteDirectResponse, Error> {
    check_service_is_available(ctx).await?;

    let forwarder = ctx.configuration.forwarder;
    let gas_tank_address = ctx.configuration.gas_tank.address;

    let transaction = ExecutableTransaction {
        forwarder,
        gas_tank_address,
        parameters: request.parameters.into(),
        transaction: request.transaction.into(),
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
    Ok(ExecuteDirectResponse {
        transaction_hash: result.transaction_hash,
        tracking_id: Felt::ZERO,
    })
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::TokenPrice;
    use paymaster_starknet::testing::transaction::an_eth_transfer;
    use paymaster_starknet::testing::TestEnvironment as StarknetTestEnvironment;
    use paymaster_starknet::transaction::ExecuteFromOutsideMessage;
    use starknet::core::types::{Call, Felt};
    use starknet::signers::SigningKey;

    use crate::endpoint::build::{build_transaction_endpoint, BuildTransactionRequest, BuildTransactionResponse, InvokeParameters, TransactionParameters};
    use crate::endpoint::common::{ExecutionParameters, FeeMode, TipPriority};
    use crate::endpoint::execute_raw::{execute_direct_endpoint, DirectInvokeParameters, ExecuteDirectRequest, ExecuteDirectTransactionParameters};
    use crate::endpoint::RequestContext;
    use crate::testing::TestEnvironment;
    use crate::{Error, InvokeTransaction};

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

        // Build a transaction first to get the typed_data
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

        // Sign the message
        let message_hash = typed_data
            .message_hash(StarknetTestEnvironment::ACCOUNT_ARGENT_1.address)
            .unwrap();
        let signature = SigningKey::from_secret_scalar(StarknetTestEnvironment::ACCOUNT_ARGENT_1.private_key)
            .sign(&message_hash)
            .unwrap();

        // Build the execute_from_outside call from the typed_data
        let message = ExecuteFromOutsideMessage::from_typed_data(&typed_data).unwrap();
        let execute_from_outside_call: Call = message.to_call(StarknetTestEnvironment::ACCOUNT_ARGENT_1.address, &vec![signature.r, signature.s]);

        // set no token available
        context.price = paymaster_prices::Client::mock::<NoPriceOracle>();

        let request = ExecuteDirectRequest {
            transaction: ExecuteDirectTransactionParameters::Invoke {
                invoke: DirectInvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    execute_from_outside_call,
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

        let result = execute_direct_endpoint(&RequestContext::empty(&context), request).await;
        assert!(matches!(result, Err(Error::ServiceNotAvailable)))
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    async fn execute_raw_works_properly() {
        let test = TestEnvironment::new().await;
        let request_context = RequestContext::empty(&test.context());

        // Build a transaction first to get the typed_data
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

        // Sign the message
        let message_hash = typed_data
            .message_hash(StarknetTestEnvironment::ACCOUNT_ARGENT_1.address)
            .unwrap();
        let signature = SigningKey::from_secret_scalar(StarknetTestEnvironment::ACCOUNT_ARGENT_1.private_key)
            .sign(&message_hash)
            .unwrap();

        // Build the execute_from_outside call from the typed_data
        let message = ExecuteFromOutsideMessage::from_typed_data(&typed_data).unwrap();
        let execute_from_outside_call: Call = message.to_call(StarknetTestEnvironment::ACCOUNT_ARGENT_1.address, &vec![signature.r, signature.s]);

        let request = ExecuteDirectRequest {
            transaction: ExecuteDirectTransactionParameters::Invoke {
                invoke: DirectInvokeParameters {
                    user_address: StarknetTestEnvironment::ACCOUNT_ARGENT_1.address,
                    execute_from_outside_call,
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

        let result = execute_direct_endpoint(&request_context, request).await;
        assert!(result.is_ok())
    }
}
