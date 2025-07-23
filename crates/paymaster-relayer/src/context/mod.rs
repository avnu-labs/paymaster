use paymaster_starknet::Client;

use crate::lock::LockLayer;
use crate::rebalancing::RelayerManagerConfiguration;

pub mod configuration;
use paymaster_common::service::messaging::Messages;

mod relayers;
pub use relayers::Relayers;

use crate::Message;

#[derive(Clone)]
pub struct Context {
    pub configuration: RelayerManagerConfiguration,
    pub starknet: Client,
    pub relayers: Relayers,
    pub relayers_locks: LockLayer,
    pub messages: Messages<Message>,
}

impl Context {
    pub fn new(configuration: RelayerManagerConfiguration) -> Self {
        // Validate configuration before creating context
        if let Err(e) = configuration.validate() {
            panic!("Configuration validation failed: {}", e);
        }

        let starknet = Client::new(&configuration.starknet);
        let messages = Messages::new();
        let relayers = Relayers::new(&starknet, messages.clone(), &configuration.relayers);
        Self {
            starknet,
            relayers,
            relayers_locks: LockLayer::new(&configuration),
            configuration,
            messages,
        }
    }
}
