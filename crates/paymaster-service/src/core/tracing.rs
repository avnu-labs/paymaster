use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::Layer;

use crate::core::context::configuration::VerbosityConfiguration;

pub struct Fmt;

impl Fmt {
    pub fn layer<S>(verbosity: &VerbosityConfiguration) -> impl Layer<S>
    where
        S: for<'span> tracing_subscriber::registry::LookupSpan<'span> + tracing::Subscriber,
    {
        let filter = match verbosity {
            VerbosityConfiguration::Info => LevelFilter::INFO,
            VerbosityConfiguration::Debug => LevelFilter::DEBUG,
        };

        tracing_subscriber::fmt::layer().with_ansi(false).compact().with_filter(filter)
    }
}
