use std::borrow::Cow;

use futures::future::BoxFuture;
use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::types::Request;
use jsonrpsee::MethodResponse;

/// Enforce positional form for parameters (i.e force them to appear as an array)
/// This ensure that both the RPC Client and HTTP Raw Call are supported
#[derive(Clone)]
pub struct PayloadFormatter<S> {
    service: S,
}

impl<S> PayloadFormatter<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }

    fn wrap_parameters<'a>(&self, mut request: Request<'a>) -> Request<'a> {
        let Some(params) = request.params.clone() else {
            return request;
        };

        let payload = params.get();
        // If the request is already in positionnal form  (i.e array) do nothing
        if payload.starts_with("[") && payload.ends_with("]") {
            return request;
        }

        // Otherwise wrap payload into array
        let Ok(payload) = serde_json::value::to_raw_value(&vec![params]) else {
            return request;
        };

        request.params = Some(Cow::Owned(payload));
        request
    }
}

impl<'a, S> RpcServiceT<'a> for PayloadFormatter<S>
where
    S: RpcServiceT<'a> + Send + Sync + Clone + 'static,
{
    type Future = BoxFuture<'a, MethodResponse>;

    fn call(&self, request: Request<'a>) -> Self::Future {
        let service = self.service.clone();
        let request = self.wrap_parameters(request);

        Box::pin(async move { service.call(request).await })
    }
}
