use crate::endpoint::RequestContext;
use crate::Error;

pub async fn is_available_endpoint(ctx: &RequestContext<'_>) -> Result<bool, Error> {
    let at_least_one_relayer = ctx.context.execution.get_relayer_manager().count_enabled_relayers().await > 0;
    Ok(at_least_one_relayer)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use paymaster_prices::mock::MockPriceOracle;
    use paymaster_prices::TokenPrice;
    use starknet::core::types::Felt;

    use crate::endpoint::health::is_available_endpoint;
    use crate::endpoint::RequestContext;
    use crate::testing::TestEnvironment;

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
    async fn is_available_returns_true() {
        let test = TestEnvironment::new().await;
        let request_context = RequestContext::empty(&test.context());

        let result = is_available_endpoint(&request_context).await.unwrap();
        assert!(result)
    }

    #[tokio::test]
    async fn is_available_returns_false() {
        let test = TestEnvironment::new().await;

        let mut context = test.context().clone();
        context.price = paymaster_prices::Client::mock::<NoPriceOracle>();

        let request_context = RequestContext::empty(&context);

        let result = is_available_endpoint(&request_context).await.unwrap();
        assert!(!result)
    }
}
