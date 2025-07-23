use paymaster_common::service::monitoring::Metric;
use paymaster_common::service::{Error, ServiceManager};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use crate::core::context::Context;
use crate::core::Fmt;
use crate::rpc::RPCService;

mod core;
mod rpc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let context = Context::load()?;

    let metric_layer = context.configuration.prometheus.clone().map(|x| Metric::layer(&x));
    let fmt_layer = Fmt::layer(&context.configuration.verbosity);

    let subscriber = Registry::default().with(fmt_layer).with(metric_layer);

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let mut services = ServiceManager::new(context);
    info!("starting services...");
    services.spawn::<RPCService>();

    info!("all services started");
    services.wait()
}
