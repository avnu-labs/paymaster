use std::collections::HashMap;

use paymaster_common::{measure_duration, metric};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use thiserror::Error;
use tracing::{error, warn};

use crate::self_sponsoring::SelfSponsoring;
use crate::webhook_sponsoring::WebhookSponsoring;
mod self_sponsoring;
mod webhook_sponsoring;

#[macro_export]
macro_rules! log_if_error {
    ($e: expr) => {{
        let result = $e;
        match &result {
            Err(e@Error::HTTP(_)) => error!(message=%e),
            Err(e@Error::Internal(_)) => error!(message=%e),
            Err(e@Error::Format(_)) => error!(message=%e),
            Err(e@Error::URL(_)) => error!(message=%e),
            Err(e) => warn!(message=%e),
            _ => ()
        };
        result
    }};
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid api key: {0}")]
    InvalidApiKey(String),

    #[error(transparent)]
    HTTP(#[from] reqwest::Error),

    #[error("invalid url {0}")]
    URL(String),

    #[error("Authentication error: {0}")]
    Internal(String),

    #[error("wrong format error {0}")]
    Format(String),
}

#[derive(Debug, Default, Clone)]
pub struct AuthenticatedApiKey {
    pub is_valid: bool,
    pub sponsor_metadata: Vec<Felt>,
}
impl AuthenticatedApiKey {
    pub fn valid(sponsor_metadata: Vec<Felt>) -> Self {
        Self {
            is_valid: true,
            sponsor_metadata,
        }
    }

    pub fn invalid() -> Self {
        Self {
            is_valid: false,
            sponsor_metadata: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SelfConfiguration {
    pub api_key: String,
    pub sponsor_metadata: Vec<Felt>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WebhookConfiguration {
    endpoint: String,
    headers: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Configuration {
    None,
    #[serde(rename = "self")]
    SelfSponsoring(SelfConfiguration),
    Webhook(WebhookConfiguration),
}

impl Configuration {
    pub fn none() -> Self {
        Self::None
    }
}

#[derive(Clone)]
pub enum Authentication {
    None,
    SelfSponsoring(SelfSponsoring),
    Webhook(WebhookSponsoring),
}

#[derive(Clone)]
pub struct Client {
    authentication: Authentication,
}

impl Client {
    pub fn new(configuration: &Configuration) -> Self {
        let authentication = match configuration {
            Configuration::None => Authentication::None,
            Configuration::SelfSponsoring(config) => Authentication::SelfSponsoring(SelfSponsoring::new(config.clone()).unwrap()),
            Configuration::Webhook(config) => Authentication::Webhook(WebhookSponsoring::new(config.clone())),
        };
        Self { authentication }
    }

    pub async fn validate(&self, key: &str) -> Result<AuthenticatedApiKey, Error> {
        let (result, duration) = measure_duration!(log_if_error!(match &self.authentication {
            Authentication::None => Ok(AuthenticatedApiKey::invalid()),
            Authentication::SelfSponsoring(authentication) => Ok(authentication.validate(key)),
            Authentication::Webhook(authentication) => authentication.validate(key).await,
        }));

        metric!(counter[paymaster_sponsor_validation_request] = 1, method = "is_valid");
        metric!(histogram[paymaster_auth_request_duration_milliseconds] = duration.as_millis(), method = "is_valid");

        result
    }
}
