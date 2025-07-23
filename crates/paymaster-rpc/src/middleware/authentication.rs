use std::ops::Deref;
use std::task::{Context, Poll};

use jsonrpsee::server::{HttpBody, HttpRequest, HttpResponse};
use tower::{Layer, Service};

#[derive(Debug, Clone, Default)]
pub struct APIKey(String);

#[cfg(test)]
impl APIKey {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Deref for APIKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticationLayer;

impl<S> Layer<S> for AuthenticationLayer {
    type Service = Authentication<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Authentication { inner }
    }
}

#[derive(Debug, Clone)]
pub struct Authentication<S> {
    inner: S,
}

impl<S> Service<HttpRequest<HttpBody>> for Authentication<S>
where
    S: Service<HttpRequest, Response = HttpResponse<HttpBody>>,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: HttpRequest<HttpBody>) -> Self::Future {
        let api_key_header = req
            .headers()
            .get("x-paymaster-api-key")
            .and_then(|x| x.to_str().ok())
            .map(|x| APIKey(x.to_string()));

        if let Some(api_key) = api_key_header {
            req.extensions_mut().insert(api_key);
        }

        self.inner.call(req)
    }
}
