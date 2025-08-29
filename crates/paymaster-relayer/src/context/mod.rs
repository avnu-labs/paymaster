use paymaster_starknet::Client;

use crate::lock::LockLayer;
use crate::rebalancing::RelayerManagerConfiguration;

pub mod configuration;

mod relayers;
pub use relayers::Relayers;

#[derive(Clone)]
pub struct Context {
    pub configuration: RelayerManagerConfiguration,
    pub starknet: Client,
    pub relayers: Relayers,
    pub relayers_locks: LockLayer,
}

impl Context {
    pub fn new(configuration: RelayerManagerConfiguration) -> Self {
        // Validate configuration before creating context
        if let Err(e) = configuration.validate() {
            panic!("Configuration validation failed: {}", e);
        }

        let starknet = Client::new(&configuration.starknet);
        let relayers = Relayers::new(&starknet, &configuration.relayers);
        Self {
            starknet,
            relayers,
            relayers_locks: LockLayer::new(&configuration),
            configuration,
        }
    }
}
