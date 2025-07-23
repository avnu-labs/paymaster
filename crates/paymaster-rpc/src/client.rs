use jsonrpsee::http_client::HttpClient;

use crate::{BuildTransactionRequest, BuildTransactionResponse, ExecuteRequest, ExecuteResponse, PaymasterAPIClient, TokenPrice};

pub type Error = jsonrpsee::core::ClientError;

pub struct Client {
    inner: HttpClient,
}

impl Client {
    pub fn new(endpoint: &str) -> Self {
        Self {
            inner: HttpClient::builder().build(endpoint).expect("invalid endpoint"),
        }
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

    pub async fn get_supported_tokens(&self) -> Result<Vec<TokenPrice>, Error> {
        self.inner.get_supported_tokens().await
    }
}
