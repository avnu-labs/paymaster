use std::time::Duration;

use jsonrpsee::http_client::{HeaderMap, HeaderValue, HttpClient, HttpClientBuilder};

use crate::endpoint::execute_raw::{ExecuteDirectRequest, ExecuteDirectResponse};
use crate::{BuildTransactionRequest, BuildTransactionResponse, ExecuteRequest, ExecuteResponse, PaymasterAPIClient, TokenPrice};

pub type Error = jsonrpsee::core::ClientError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Client {
    inner: HttpClient,
}

impl Client {
    pub fn new(endpoint: &str) -> Self {
        Self {
            inner: HttpClient::builder().build(endpoint).expect("invalid endpoint"),
        }
    }

    pub fn builder(endpoint: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            endpoint: endpoint.into(),
            api_key: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub async fn health(&self) -> Result<bool, Error> {
        self.inner.health().await
    }

    pub async fn is_available(&self) -> Result<bool, Error> {
        self.inner.is_available().await
    }

    pub async fn build_transaction(&self, params: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
        self.inner.build_transaction(params).await
    }

    pub async fn execute_transaction(&self, params: ExecuteRequest) -> Result<ExecuteResponse, Error> {
        self.inner.execute_transaction(params).await
    }

    pub async fn execute_direct_transaction(&self, params: ExecuteDirectRequest) -> Result<ExecuteDirectResponse, Error> {
        self.inner.execute_direct_transaction(params).await
    }

    pub async fn get_supported_tokens(&self) -> Result<Vec<TokenPrice>, Error> {
        self.inner.get_supported_tokens().await
    }
}

pub struct ClientBuilder {
    endpoint: String,
    api_key: Option<String>,
    timeout: Duration,
}

impl ClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let mut builder = HttpClientBuilder::default().request_timeout(self.timeout);

        if let Some(key) = self.api_key {
            let mut headers = HeaderMap::new();
            headers.insert(
                "x-paymaster-api-key",
                HeaderValue::from_str(&key).map_err(|e| Error::Custom(format!("invalid API key header value: {e}")))?,
            );
            builder = builder.set_headers(headers);
        }

        let inner = builder.build(&self.endpoint)?;
        Ok(Client { inner })
    }
}
