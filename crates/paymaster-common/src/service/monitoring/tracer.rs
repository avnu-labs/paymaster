use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::level_filters::LevelFilter;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::service::monitoring::Configuration;

pub struct Tracer;

impl Tracer {
    pub fn layer<S>(configuration: &Configuration) -> impl Layer<S>
    where
        S: Subscriber,
        S: for<'span> LookupSpan<'span>,
    {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/traces", configuration.endpoint))
            .with_protocol(Protocol::HttpBinary)
            .with_headers(configuration.headers())
            .build()
            .unwrap();

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(Resource::builder().with_service_name("paymaster").build())
            .build();

        let tracer = provider.tracer("paymaster");
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(LevelFilter::TRACE)
    }
}
