use async_trait::async_trait;
use paymaster_common::service::{Error, Service};
use paymaster_rpc::server::PaymasterServer;

use crate::core::context::Context;

pub struct RPCService {
    context: Context,
}

#[async_trait]
impl Service for RPCService {
    type Context = Context;

    const NAME: &'static str = "RPC";

    async fn new(context: Context) -> Self {
        Self { context }
    }

    async fn run(mut self) -> Result<(), Error> {
        let server = PaymasterServer::new(&self.context.into());
        let handle = server.start().await?;

        handle.stopped().await;

        Err(Error::new("rpc server stopped unexpectedly"))
    }
}
