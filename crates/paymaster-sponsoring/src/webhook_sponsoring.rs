use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use paymaster_common::concurrency::SyncValue;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use tokio::sync::RwLock;

use crate::{AuthenticatedApiKey, Error, WebhookConfiguration};

#[derive(Serialize, Deserialize)]
struct ApiKeyValidationResponse {
    is_valid: bool,
    sponsor_metadata: Vec<Felt>,
    validity_duration: u64,
}

#[derive(Clone)]
pub struct WebhookSponsoring {
    endpoint: String,
    headers: HeaderMap,
    client: Client,
    cache: Arc<RwLock<HashMap<String, SyncValue<AuthenticatedApiKey>>>>,
}

impl WebhookSponsoring {
    pub fn new(configuration: WebhookConfiguration) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .expect("Failed to build HTTP client");
        let headers = configuration
            .headers
            .into_iter()
            .filter_map(|(k, v)| {
                let name = HeaderName::from_bytes(k.as_bytes()).ok()?;
                let value = HeaderValue::from_str(&v).ok()?;
                Some((name, value))
            })
            .collect::<HeaderMap>();
        Self {
            endpoint: configuration.endpoint,
            headers,
            client,
            cache: Arc::default(),
        }
    }

    pub async fn validate(&self, api_key: &str) -> Result<AuthenticatedApiKey, Error> {
        let cached_status = self.get_or_insert_cache_entry(api_key).await;
        let this = self.clone();
        let key = api_key.to_owned();
        cached_status
            .read_or_refresh_with_ttl({
                move || {
                    Box::pin(async move {
                        let response = this.fetch_validate(&key).await?;
                        Ok((
                            AuthenticatedApiKey {
                                is_valid: response.is_valid,
                                sponsor_metadata: response.sponsor_metadata,
                            },
                            response.validity_duration,
                        ))
                    })
                }
            })
            .await
    }

    async fn get_or_insert_cache_entry(&self, api_key: &str) -> SyncValue<AuthenticatedApiKey> {
        if let Some(value) = self.cache.read().await.get(&api_key.to_string()) {
            return value.clone();
        }

        let mut write_lock = self.cache.write().await;
        write_lock
            .entry(api_key.to_string())
            .or_insert(SyncValue::new(Duration::from_secs(60)))
            .clone()
    }

    async fn fetch_validate(&self, api_key: &str) -> Result<ApiKeyValidationResponse, Error> {
        let url = Url::parse(&self.endpoint).map_err(|e| Error::URL(e.to_string()))?;
        let mut headers = self.headers.clone();
        headers.insert("x-paymaster-api-key", HeaderValue::from_str(api_key).map_err(|e| Error::Internal(e.to_string()))?);

        let response = self.client.get(url).headers(headers).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(Error::Internal(format!("Api key validation request error status={}, body={}", status, text)));
        }

        serde_json::from_str::<ApiKeyValidationResponse>(&text).map_err(|e| Error::Format(e.to_string()))
    }
}
