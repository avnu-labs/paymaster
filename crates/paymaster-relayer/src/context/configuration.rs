use paymaster_common::service::Error as ServiceError;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::Felt;

use crate::lock::LockLayerConfiguration;
use crate::rebalancing::OptionalRebalancingConfiguration;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayersConfiguration {
    #[serde_as(as = "UfeHex")]
    pub private_key: Felt,

    #[serde_as(as = "Vec<UfeHex>")]
    pub addresses: Vec<Felt>,

    #[serde_as(as = "UfeHex")]
    pub min_relayer_balance: Felt,

    pub lock: LockLayerConfiguration,

    #[serde(default)]
    pub rebalancing: OptionalRebalancingConfiguration,
}

impl RelayersConfiguration {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.addresses.is_empty() {
            return Err(ServiceError::new("At least one relayer address must be configured"));
        }

        // Validate rebalancing configuration (including trigger_balance > min_relayer_balance)
        self.rebalancing.validate(self.min_relayer_balance)?;

        Ok(())
    }
}
