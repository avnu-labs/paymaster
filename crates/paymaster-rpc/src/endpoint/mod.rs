use std::ops::Deref;

use bigdecimal::Zero;
use hyper::http::Extensions;
use paymaster_prices::TokenPrice;
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
        let tokens = self
            .context
            .price
            .fetch_tokens(&self.context.configuration.supported_tokens)
            .await?
            .into_iter()
            .filter(|x| !x.price_in_strk.is_zero())
            .collect();

        Ok(tokens)
    }
}
