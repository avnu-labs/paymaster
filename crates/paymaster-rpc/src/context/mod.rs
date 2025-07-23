mod configuration;
pub use configuration::{Configuration, RPCConfiguration};
use paymaster_execution::Client as ExecutionClient;
use paymaster_prices::Client as PriceClient;
use paymaster_sponsoring::Client as SponsoringClient;

#[derive(Clone)]
pub struct Context {
    pub configuration: Configuration,

    pub price: PriceClient,
    pub sponsoring: SponsoringClient,

    pub execution: ExecutionClient,
}

impl Context {
    pub fn new(configuration: Configuration) -> Self {
        Self {
            price: PriceClient::new(&configuration.price),
            sponsoring: SponsoringClient::new(&configuration.sponsoring),

            execution: ExecutionClient::new(&configuration.clone().into()),

            configuration,
        }
    }
}
