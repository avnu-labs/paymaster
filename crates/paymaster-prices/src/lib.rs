use std::collections::HashSet;

use async_trait::async_trait;
use paymaster_common::concurrency::ConcurrentExecutor;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use thiserror::Error;

use crate::avnu::{AVNUPriceClientConfiguration, AVNUPriceOracle};

pub mod avnu;

pub mod math;

#[cfg(feature = "testing")]
pub mod mock;

use paymaster_common::service::tracing::instrument;
use paymaster_common::{log_if_error, measure_duration, metric, task};

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

    #[error("Price error: {0}")]
    Internal(String),
}

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenPrice {
    pub address: Felt,
    pub decimals: i64,

    pub price_in_strk: Felt,
}

#[async_trait]
pub trait PriceOracle: 'static + Send + Sync + Clone {
    async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, Error>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum Configuration {
    #[cfg(feature = "testing")]
    #[serde(skip)]
    Mock(std::sync::Arc<dyn mock::MockPriceOracle>),

    #[serde(rename = "avnu")]
    AVNU(AVNUPriceClientConfiguration),
}

#[cfg(feature = "testing")]
impl Configuration {
    pub fn mock<T: mock::MockPriceOracle>() -> Self {
        Self::Mock(std::sync::Arc::new(T::new()))
    }
}

#[derive(Clone)]
pub enum Client {
    #[cfg(feature = "testing")]
    Mock(std::sync::Arc<dyn mock::MockPriceOracle>),

    AVNU(AVNUPriceOracle),
}

impl Client {
    pub fn new(configuration: &Configuration) -> Self {
        match configuration {
            #[cfg(feature = "testing")]
            Configuration::Mock(x) => Self::Mock(x.clone()),

            Configuration::AVNU(x) => Self::AVNU(AVNUPriceOracle::new(x)),
        }
    }

    #[cfg(feature = "testing")]
    pub fn mock<I: 'static + mock::MockPriceOracle>() -> Self {
        Self::Mock(std::sync::Arc::new(I::new()))
    }

    pub async fn convert_token_to_strk(&self, token: Felt, amount: Felt) -> Result<Felt, Error> {
        let token_price = self.fetch_token(token).await?;

        convert_token_to_strk(&token_price, amount)
    }

    pub async fn convert_strk_to_token(&self, token: Felt, amount: Felt, round_up: bool) -> Result<Felt, Error> {
        let token_price = self.fetch_token(token).await?;

        convert_strk_to_token(&token_price, amount, round_up)
    }

    pub async fn fetch_tokens(&self, tokens: &HashSet<Felt>) -> Result<Vec<TokenPrice>, Error> {
        let mut executor = ConcurrentExecutor::new(self.clone(), 8);
        for token in tokens.iter().cloned() {
            executor.register(task!(|context| { context.fetch_token(token).await }));
        }

        let mut tokens = Vec::with_capacity(tokens.len());
        while let Some(result) = executor.next().await {
            tokens.push(result.map_err(|e| Error::Internal(e.to_string()))??);
        }

        Ok(tokens)
    }

    #[instrument(name = "fetch_token", skip(self))]
    pub async fn fetch_token(&self, address: Felt) -> Result<TokenPrice, Error> {
        let (result, duration) = measure_duration!(log_if_error!(match self {
            #[cfg(feature = "testing")]
            Self::Mock(oracle) => oracle.fetch_token(address).await,

            Self::AVNU(oracle) => oracle.fetch_token(address).await,
        }));

        metric!(counter[price_request] = 1, method = "fetch_token");
        metric!(histogram[price_request_duration_milliseconds] = duration.as_millis(), method = "fetch_token");
        metric!(on error result => counter [ price_request_error ] = 1, method = "fetch_token");

        result
    }
}
