use opentelemetry::global;
use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::Resource;
use tracing::Subscriber;
use tracing_opentelemetry::MetricsLayer;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::service::monitoring::Configuration;

#[macro_export]
macro_rules! measure_duration {
    ($e: expr) => {{
        let now = std::time::Instant::now();

        let result = $e;
        (result, now.elapsed())
    }};
}

#[macro_export]
macro_rules! metric {
    (counter [ $label: ident ] = $i: expr $(,$field: ident = $value: expr)*) => {
        tracing::debug!(monotonic_counter.$label = $i, $($field = $value),*)
    };
    (on error $e: expr => counter [ $label: ident ] = $i: expr $(,$field: ident = $value: expr)*) => {
        if let Err(ref e) = $e {
            tracing::debug!(counter.$label = $i, $($field = $value,)* error = e.to_string());
        }
    };
    (gauge [ $label: ident ] = $i: expr $(,$field: ident = $value: expr)*) => {
        tracing::debug!(gauge.$label = $i, $($field = $value),*)
    };
    (histogram [ $label: ident ] = $i: expr $(,$field: ident = $value: expr)*) => {
        tracing::debug!(histogram.$label = $i as f64, $($field = $value),*)
    };
}

pub struct Metric;

impl Metric {
    pub fn layer<S>(configuration: &Configuration) -> impl Layer<S>
    where
        S: Subscriber,
        S: for<'span> LookupSpan<'span>,
    {
        let exporter = MetricExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/metrics", configuration.endpoint))
            .with_protocol(Protocol::HttpBinary)
            .with_headers(configuration.headers())
            .build()
            .expect("could not build metric exporter");

        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter)
            .with_resource(Resource::builder().with_service_name("paymaster").build())
            .build();

        global::set_meter_provider(provider.clone());

        MetricsLayer::new(provider)
    }
}
