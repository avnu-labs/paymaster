use jsonrpsee::core::Serialize;
use serde::Deserialize;
use starknet::core::types::Felt;

use crate::endpoint::RequestContext;
use crate::Error;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct TokenPrice {
    pub token_address: Felt,
    pub decimals: i64,

    pub price_in_strk: Felt,
}

impl From<paymaster_prices::TokenPrice> for TokenPrice {
    fn from(value: paymaster_prices::TokenPrice) -> Self {
        Self {
            token_address: value.address,
            decimals: value.decimals,
            price_in_strk: value.price_in_strk,
        }
    }
}

pub async fn get_supported_tokens_endpoint(ctx: &RequestContext<'_>) -> Result<Vec<TokenPrice>, Error> {
    let tokens = ctx.fetch_available_tokens().await?.into_iter().map(|x| x.into()).collect();

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use async_trait::async_trait;
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::TokenPrice;
    use starknet::core::types::Felt;

    use crate::endpoint::token::get_supported_tokens_endpoint;
    use crate::endpoint::RequestContext;
    use crate::testing::{StarknetTestEnvironment, TestEnvironment};

    #[derive(Debug, Clone)]
    struct PriceOracle;

    #[async_trait]
    impl MockPriceOracle for PriceOracle {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, paymaster_prices::Error> {
            Ok(match address {
                x if x == StarknetTestEnvironment::ETH => TokenPrice {
                    address: StarknetTestEnvironment::ETH,
                    price_in_strk: Felt::ONE,
                    decimals: 18,
                },
                x if x == StarknetTestEnvironment::USDC => TokenPrice {
                    address: StarknetTestEnvironment::ETH,
                    price_in_strk: Felt::ZERO,
                    decimals: 18,
                },
                _ => unimplemented!(),
            })
        }
    }

    #[tokio::test]
    async fn get_supported_tokens_works_properly() {
        let test = TestEnvironment::new().await;

        let mut context = test.context().clone();
        context.configuration.supported_tokens = HashSet::from([StarknetTestEnvironment::ETH, StarknetTestEnvironment::USDC]);
        context.price = paymaster_prices::Client::mock::<PriceOracle>();

        let request_context = RequestContext::empty(&context);

        let results = get_supported_tokens_endpoint(&request_context).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].token_address, StarknetTestEnvironment::ETH)
    }
}
