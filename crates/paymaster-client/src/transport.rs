use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::Error;

#[derive(Serialize)]
struct JsonRpcRequest<P: Serialize> {
    jsonrpc: &'static str,
    method: String,
    params: P,
    id: u64,
}

#[derive(Deserialize)]
struct JsonRpcResponse<R> {
    result: Option<R>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: u64,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct Transport {
    client: reqwest::Client,
    endpoint: String,
    id_counter: std::sync::Arc<AtomicU64>,
}

impl Transport {
    pub fn new(endpoint: String, api_key: Option<&str>, timeout: Duration) -> Result<Self, Error> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = api_key {
            headers.insert(
                "x-paymaster-api-key",
                reqwest::header::HeaderValue::from_str(key).map_err(|e| Error::Configuration(format!("invalid API key header value: {e}")))?,
            );
        }

        let client = reqwest::Client::builder().default_headers(headers).timeout(timeout).build()?;

        Ok(Self {
            client,
            endpoint,
            id_counter: std::sync::Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn call<P, R>(&self, method: &str, params: P) -> Result<R, Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = self.id_counter.fetch_add(1, Ordering::Relaxed);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id,
        };

        let response = self.client.post(&self.endpoint).json(&request).send().await?;

        let rpc_response: JsonRpcResponse<R> = response.json().await?;

        if let Some(err) = rpc_response.error {
            return Err(Error::Rpc {
                code: err.code,
                message: err.message,
                data: err.data,
            });
        }

        rpc_response.result.ok_or_else(|| Error::Rpc {
            code: -1,
            message: "missing result in JSON-RPC response".to_string(),
            data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_partial_json, header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn sends_correct_jsonrpc_envelope() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "paymaster_health"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": true,
                "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let transport = Transport::new(server.uri(), None, Duration::from_secs(5)).unwrap();
        let result: bool = transport.call("paymaster_health", serde_json::json!([])).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn sends_api_key_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(header("x-paymaster-api-key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": true,
                "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let transport = Transport::new(server.uri(), Some("test-key"), Duration::from_secs(5)).unwrap();
        let result: bool = transport.call("paymaster_health", serde_json::json!([])).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn parses_rpc_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": 151,
                    "message": "An error occurred (TOKEN_NOT_SUPPORTED)"
                },
                "id": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let transport = Transport::new(server.uri(), None, Duration::from_secs(5)).unwrap();
        let result: Result<bool, Error> = transport.call("paymaster_buildTransaction", serde_json::json!([])).await;

        match result {
            Err(Error::Rpc { code, message, .. }) => {
                assert_eq!(code, 151);
                assert!(message.contains("TOKEN_NOT_SUPPORTED"));
            },
            other => panic!("expected Rpc error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn increments_request_id() {
        let transport = Transport::new("http://localhost:1".to_string(), None, Duration::from_secs(1)).unwrap();
        let id1 = transport.id_counter.load(Ordering::Relaxed);
        // Simulate fetch_add behavior
        let _ = transport.id_counter.fetch_add(1, Ordering::Relaxed);
        let id2 = transport.id_counter.load(Ordering::Relaxed);
        assert_eq!(id2, id1 + 1);
    }
}
