use std::collections::HashSet;
use std::ops::Deref;

use bigdecimal::Zero;
use hyper::http::Extensions;
use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::task;
use paymaster_prices::{Client as PriceClient, TokenPrice};
use paymaster_sponsoring::AuthenticatedApiKey;

use crate::context::Context;
pub use crate::middleware::APIKey;
use crate::Error;

pub mod build;
pub mod common;
pub mod execute;
pub mod health;
pub mod token;
mod validation;

pub struct RequestContext<'a> {
    context: &'a Context,

    pub api_key: Option<APIKey>,
}

impl Deref for RequestContext<'_> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.context
    }
}

impl<'a> RequestContext<'a> {
    pub fn new(ctx: &'a Context, extensions: &Extensions) -> Self {
        Self {
            context: ctx,
            api_key: extensions.get::<APIKey>().cloned(),
        }
    }

    #[cfg(test)]
    pub fn empty(ctx: &'a Context) -> Self {
        Self { context: ctx, api_key: None }
    }

    pub async fn validate_api_key(&self) -> Result<AuthenticatedApiKey, Error> {
        let key = self.api_key.clone().unwrap_or_default();
        let authenticated_api_key = self.sponsoring.validate(&key).await.map_err(|_| Error::InvalidAPIKey)?;

        if authenticated_api_key.is_valid {
            return Ok(authenticated_api_key);
        }

        Err(Error::InvalidAPIKey)
    }

    pub async fn fetch_available_tokens(&self) -> Result<Vec<TokenPrice>, Error> {
        let mut executor: ConcurrentExecutor<PriceClient, Option<TokenPrice>> = ConcurrentExecutor::new(self.context.price.clone(), 8);

        for token in &self.context.configuration.supported_tokens {
            let token = *token;
            executor.register(task!(|price| {
                price
                    .fetch_tokens(&HashSet::from([token]))
                    .await
                    .ok()
                    .and_then(|prices| prices.into_iter().next())
                    .filter(|tp| !tp.price_in_strk.is_zero())
            }));
        }

        Ok(executor.execute().await.unwrap_or_default().into_iter().flatten().collect())
    }
}
