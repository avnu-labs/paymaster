use std::time::Duration;

use tracing::instrument;

use crate::transaction::TransactionBuilder;
use crate::transport::Transport;
use crate::types::*;
use crate::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Low-level client for the avnu Paymaster JSON-RPC API.
///
/// Provides 1:1 mappings to all RPC methods, plus a high-level
/// [`transaction()`](PaymasterClient::transaction) builder for the build-sign-execute flow.
#[derive(Clone)]
pub struct PaymasterClient {
    transport: Transport,
}

impl PaymasterClient {
    /// Creates a new client builder with the given endpoint URL.
    pub fn builder(endpoint: impl Into<String>) -> PaymasterClientBuilder {
        PaymasterClientBuilder {
            endpoint: endpoint.into(),
            api_key: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Creates a new client with default settings (no API key, 30s timeout).
    pub fn new(endpoint: impl Into<String>) -> Result<Self, Error> {
        Self::builder(endpoint).build()
    }

    #[instrument(skip(self))]
    pub async fn health(&self) -> Result<bool, Error> {
        self.transport.call("paymaster_health", serde_json::json!([])).await
    }

    #[instrument(skip(self))]
    pub async fn is_available(&self) -> Result<bool, Error> {
        self.transport.call("paymaster_isAvailable", serde_json::json!([])).await
    }

    #[instrument(skip(self))]
    pub async fn get_supported_tokens(&self) -> Result<Vec<TokenPrice>, Error> {
        self.transport.call("paymaster_getSupportedTokens", serde_json::json!([])).await
    }

    #[instrument(skip(self, req))]
    pub async fn build_transaction(&self, req: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
        self.transport.call("paymaster_buildTransaction", vec![req]).await
    }

    #[instrument(skip(self, req))]
    pub async fn execute_transaction(&self, req: ExecuteRequest) -> Result<ExecuteResponse, Error> {
        self.transport.call("paymaster_executeTransaction", vec![req]).await
    }

    #[instrument(skip(self, req))]
    pub async fn execute_direct_transaction(&self, req: ExecuteDirectRequest) -> Result<ExecuteDirectResponse, Error> {
        self.transport.call("paymaster_executeDirectTransaction", vec![req]).await
    }

    /// Returns a high-level builder that orchestrates build, sign, and execute.
    pub fn transaction(&self) -> TransactionBuilder<'_> {
        TransactionBuilder::new(self)
    }
}

/// Builder for configuring a [`PaymasterClient`].
pub struct PaymasterClientBuilder {
    endpoint: String,
    api_key: Option<String>,
    timeout: Duration,
}

impl PaymasterClientBuilder {
    /// Sets the API key sent via the `x-paymaster-api-key` header.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the HTTP request timeout (default: 30s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds the [`PaymasterClient`].
    pub fn build(self) -> Result<PaymasterClient, Error> {
        let transport = Transport::new(self.endpoint, self.api_key.as_deref(), self.timeout)?;
        Ok(PaymasterClient { transport })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn health_sends_correct_method() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_health"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "result": true, "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(server.uri()).unwrap();
        assert!(client.health().await.unwrap());
    }

    #[tokio::test]
    async fn is_available_sends_correct_method() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_isAvailable"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "result": true, "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(server.uri()).unwrap();
        assert!(client.is_available().await.unwrap());
    }

    #[tokio::test]
    async fn get_supported_tokens_sends_correct_method() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({"method": "paymaster_getSupportedTokens"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "result": [], "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = PaymasterClient::new(server.uri()).unwrap();
        let tokens = client.get_supported_tokens().await.unwrap();
        assert!(tokens.is_empty());
    }

    #[tokio::test]
    async fn builder_sets_timeout() {
        let client = PaymasterClient::builder("http://localhost:1234")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        // Client builds successfully with custom timeout
        drop(client);
    }

    #[tokio::test]
    async fn client_is_clone() {
        let client = PaymasterClient::new("http://localhost:1234").unwrap();
        let _cloned = client.clone();
    }
}
