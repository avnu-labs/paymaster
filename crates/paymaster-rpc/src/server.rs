use async_trait::async_trait;
use hyper::http::Extensions;
use jsonrpsee::server::middleware::http::ProxyGetRequestLayer;
use jsonrpsee::server::{RpcServiceBuilder, ServerBuilder, ServerHandle};
use paymaster_common::service::Error as ServiceError;
use paymaster_common::{measure_duration, metric};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{error, info, instrument, warn};

use crate::context::Context;
use crate::endpoint::build::build_transaction_endpoint;
use crate::endpoint::execute::execute_endpoint;
use crate::endpoint::health::is_available_endpoint;
use crate::endpoint::token::get_supported_tokens_endpoint;
use crate::endpoint::RequestContext;
use crate::middleware::{AuthenticationLayer, PayloadFormatter};
use crate::{BuildTransactionRequest, BuildTransactionResponse, Configuration, Error, ExecuteRequest, ExecuteResponse, PaymasterAPIServer, TokenPrice};

#[macro_export]
macro_rules! log_if_error {
    ($e: expr) => {{
        let result = $e;
        match &result {
            Err(e@Error::ServiceNotAvailable) => error!(message=%e),
            Err(e) => warn!(message=%e),
            _ => ()
        };

        result
    }};
}

macro_rules! instrument_method {
    ($method: ident ($($arg: expr),*)) => {{
        metric!(counter [ rpc_request ] = 1, method = stringify!($method));

        let (result, time) = measure_duration!(log_if_error!($method($($arg),*).await));
        metric!(histogram [ rpc_request_duration_milliseconds ] = time.as_millis(), method = stringify!($method));
        metric!(on error result => counter [ rpc_request_error ] = 1);

        result
    }};
}

pub struct PaymasterServer {
    context: Context,
}

impl PaymasterServer {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            context: Context::new(configuration.clone()),
        }
    }

    pub async fn start(self) -> Result<ServerHandle, ServiceError> {
        let url = format!("0.0.0.0:{}", self.context.configuration.rpc.port);
        info!("Starting RPC server at {}", url);

        let http_middleware = ServiceBuilder::new()
            .layer(CorsLayer::permissive())
            .layer(AuthenticationLayer)
            .layer(ProxyGetRequestLayer::new("/health", "paymaster_health").unwrap());

        let rpc_middleware = RpcServiceBuilder::new().layer_fn(PayloadFormatter::new);

        let server = ServerBuilder::default()
            .max_connections(1024)
            .http_only()
            .set_http_middleware(http_middleware)
            .set_rpc_middleware(rpc_middleware)
            .build(url)
            .await
            .map_err(ServiceError::from)?;

        Ok(server.start(self.into_rpc()))
    }
}

#[async_trait]
impl PaymasterAPIServer for PaymasterServer {
    #[instrument(name = "paymaster_health", skip(self))]
    async fn health(&self, _: &Extensions) -> Result<bool, Error> {
        Ok(true)
    }

    #[instrument(name = "paymaster_isAvailable", skip(self, ext))]
    async fn is_available(&self, ext: &Extensions) -> Result<bool, Error> {
        let context = RequestContext::new(&self.context, ext);
        instrument_method!(is_available_endpoint(&context))
    }

    #[instrument(name = "paymaster_buildTransaction", skip(self, ext, params), fields(params = %serde_json::to_string(&params).unwrap_or_else(|_| "INVALID_JSON".into())))]
    async fn build_transaction(&self, ext: &Extensions, params: BuildTransactionRequest) -> Result<BuildTransactionResponse, Error> {
        let context = RequestContext::new(&self.context, ext);
        instrument_method!(build_transaction_endpoint(&context, params))
    }

    #[instrument(name = "paymaster_executeTransaction", skip(self, ext, params), fields(params = %serde_json::to_string(&params).unwrap_or_else(|_| "INVALID_JSON".into())))]
    async fn execute_transaction(&self, ext: &Extensions, params: ExecuteRequest) -> Result<ExecuteResponse, Error> {
        let context = RequestContext::new(&self.context, ext);
        instrument_method!(execute_endpoint(&context, params))
    }

    #[instrument(name = "paymaster_getSupportedTokens", skip(self, ext))]
    async fn get_supported_tokens(&self, ext: &Extensions) -> Result<Vec<TokenPrice>, Error> {
        let context = RequestContext::new(&self.context, ext);
        instrument_method!(get_supported_tokens_endpoint(&context))
    }
}
