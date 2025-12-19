use std::collections::HashSet;

use crate::avnu::{AVNUPriceClientConfiguration, AVNUPriceOracle};
use paymaster_common::concurrency::ConcurrentExecutor;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use thiserror::Error;

pub mod avnu;
pub mod coingecko;

pub mod math;

mod decimals;
#[cfg(feature = "testing")]
pub mod mock;

use paymaster_common::service::fallback::{FailurePredicate, WithFallback};
use paymaster_common::service::tracing::instrument;
use paymaster_common::{log_if_error, measure_duration, metric, task};

use crate::coingecko::{CoingeckoPriceClient, CoingeckoPriceClientConfiguration};
use crate::math::{convert_strk_to_token, convert_token_to_strk};

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid url {0}")]
    URL(String),

    #[error(transparent)]
    HTTP(#[from] reqwest::Error),

    #[error("wrong format error {0}")]
    Format(String),

    #[error("price is invalid {0}")]
    InvalidPrice(Felt),

    #[error("decimals are invalid {0}")]
    InvalidDecimals(Felt),

    #[error("Price error: {0}")]
    Internal(String),
}

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenPrice {
    pub address: Felt,
    pub decimals: i64,
    pub price_in_strk: Felt,
}

#[derive(Clone, Debug)]
pub struct PriceConfiguration {
    pub principal: PriceOracleConfiguration,
    pub fallbacks: Vec<PriceOracleConfiguration>,
}

#[derive(Clone, Debug)]
pub enum PriceOracleConfiguration {
    #[cfg(feature = "testing")]
    Mock(std::sync::Arc<dyn mock::MockPriceOracle>),

    AVNU(AVNUPriceClientConfiguration),
    Coingecko(CoingeckoPriceClientConfiguration),
}

#[cfg(feature = "testing")]
impl PriceOracleConfiguration {
    pub fn mock<T: mock::MockPriceOracle>() -> Self {
        Self::Mock(std::sync::Arc::new(T::new()))
    }
}

#[derive(Clone)]
pub struct Client {
    client: WithFallback<PriceClient>,
}

impl Client {
    pub fn new(configuration: &PriceConfiguration) -> Self {
        let mut client = WithFallback::new().with(PriceClient::new(&configuration.principal));

        for fallback in configuration.fallbacks.iter() {
            client = client.with(PriceClient::new(fallback));
        }

        Self { client }
    }

    #[cfg(feature = "testing")]
    pub fn mock<I: 'static + mock::MockPriceOracle>() -> Self {
        Self {
            client: WithFallback::new().with(PriceClient::mock::<I>()),
        }
    }

    pub async fn convert_token_to_strk(&self, token: Felt, amount: Felt) -> Result<Felt, Error> {
        let token_price = self.fetch_token(token).await?;

        convert_token_to_strk(&token_price, amount)
    }

    pub async fn convert_strk_to_token(&self, token: Felt, amount: Felt, round_up: bool) -> Result<Felt, Error> {
        let token_price = self.fetch_token(token).await?;

        convert_strk_to_token(&token_price, amount, round_up)
    }

    pub async fn fetch_tokens(&self, tokens: &HashSet<Felt>) -> Vec<Result<TokenPrice, Error>> {
        let mut executor = ConcurrentExecutor::new(self.clone(), 8);
        for token in tokens.iter().cloned() {
            executor.register(task!(|context| { context.fetch_token(token).await }));
        }

        let mut results = Vec::with_capacity(tokens.len());
        while let Some(result) = executor.next().await {
            results.push(result.map_err(|e| Error::Internal(e.to_string())).flatten());
        }

        results
    }

    pub async fn fetch_token(&self, token: Felt) -> Result<TokenPrice, Error> {
        self.client
            .call_all(|x| async move { x.fetch_token(token).await })
            .await
            .map_err(|_| Error::Internal("could not fetch price".to_string()))
    }
}

#[derive(Clone)]
pub enum PriceClient {
    #[cfg(feature = "testing")]
    Mock(std::sync::Arc<dyn mock::MockPriceOracle>),

    AVNU(AVNUPriceOracle),
    Coingecko(CoingeckoPriceClient),
}

impl FailurePredicate<Error> for PriceClient {
    fn is_err(&self, err: &Error) -> bool {
        matches!(err, Error::HTTP(_))
    }
}

impl PriceClient {
    pub fn new(configuration: &PriceOracleConfiguration) -> Self {
        match configuration {
            #[cfg(feature = "testing")]
            PriceOracleConfiguration::Mock(x) => Self::Mock(x.clone()),

            PriceOracleConfiguration::Coingecko(x) => Self::Coingecko(CoingeckoPriceClient::new(x)),
            PriceOracleConfiguration::AVNU(x) => Self::AVNU(AVNUPriceOracle::new(x)),
        }
    }

    #[cfg(feature = "testing")]
    pub fn mock<I: 'static + mock::MockPriceOracle>() -> Self {
        Self::Mock(std::sync::Arc::new(I::new()))
    }

    #[instrument(name = "fetch_token", skip(self))]
    pub async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, Error> {
        let (result, duration) = measure_duration!(log_if_error!(match self {
            #[cfg(feature = "testing")]
            Self::Mock(oracle) => oracle.fetch_token(address).await,

            Self::AVNU(oracle) => oracle.fetch_token(&address).await,
            Self::Coingecko(oracle) => oracle.fetch_token(&address).await,
        }));

        metric!(counter[price_request] = 1, method = "fetch_token");
        metric!(histogram[price_request_duration_milliseconds] = duration.as_millis(), method = "fetch_token");
        metric!(on error result => counter [ price_request_error ] = 1, method = "fetch_token");

        result
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use paymaster_starknet::constants::Token;
    use starknet::macros::felt_dec;
    use std::sync::Arc;

    use super::*;
    use crate::mock::MockPriceOracle;

    #[derive(Debug)]
    struct SuccessClient;

    #[async_trait]
    impl MockPriceOracle for SuccessClient {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn fetch_token(&self, _address: Felt) -> Result<TokenPrice, Error> {
            Ok(TokenPrice {
                address: Token::ETH_ADDRESS,
                decimals: 18,
                price_in_strk: felt_dec!("50"),
            })
        }
    }

    #[derive(Debug)]
    struct FailureClient;

    #[async_trait]
    impl MockPriceOracle for FailureClient {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }

        async fn fetch_token(&self, _address: Felt) -> Result<TokenPrice, Error> {
            Err(Error::Internal(String::new()))
        }
    }

    #[tokio::test]
    async fn should_use_fallback_properly() {
        // Given
        let oracle = Client::new(&PriceConfiguration {
            principal: PriceOracleConfiguration::Mock(Arc::new(FailureClient)),
            fallbacks: vec![
                PriceOracleConfiguration::Mock(Arc::new(FailureClient)),
                PriceOracleConfiguration::Mock(Arc::new(SuccessClient)),
            ],
        });

        // When
        let mut results = oracle.fetch_tokens(&HashSet::from([Token::ETH_ADDRESS])).await;
        let result = results.remove(0).unwrap();

        // Then
        assert_eq!(result.address, Token::ETH_ADDRESS);
        assert!(result.price_in_strk > Felt::ZERO);
    }
}
