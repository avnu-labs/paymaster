use paymaster_starknet::constants::{ClassHash, Contract};
use paymaster_starknet::transaction::CalldataBuilder;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::accounts::Account;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{BroadcastedInvokeTransactionV3, BroadcastedTransaction, Call, DataAvailabilityMode, Felt, ResourceBounds, ResourceBoundsMapping};
use starknet::macros::selector;

use crate::{Client, Error};

/// Deployment parameters required to deploy a contract
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeploymentParameters {
    #[serde_as(as = "UfeHex")]
    pub address: Felt,

    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,

    #[serde_as(as = "UfeHex")]
    pub salt: Felt,

    #[serde_as(as = "UfeHex")]
    pub unique: Felt,

    #[serde_as(as = "Vec<UfeHex>")]
    pub calldata: Vec<Felt>,

    #[serde_as(as = "Option<Vec<UfeHex>>")]
    pub sigdata: Option<Vec<Felt>>,

    pub version: u8,
}

impl DeploymentParameters {
    /// Convert the deployment parameters to a starknet transaction
    pub(crate) async fn build_transaction(&self, client: &Client) -> Result<BroadcastedTransaction, Error> {
        let estimate_account = client.estimate_account.address();
        let estimate_account_nonce = client.starknet.fetch_nonce(estimate_account).await?;

        Ok(BroadcastedTransaction::Invoke(BroadcastedInvokeTransactionV3 {
            sender_address: estimate_account,
            calldata: CalldataBuilder::new().encode(&vec![self.as_call()]).build(),
            signature: vec![],
            nonce: estimate_account_nonce,
            resource_bounds: ResourceBoundsMapping {
                l1_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
                l1_data_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
                l2_gas: ResourceBounds {
                    max_amount: 0,
                    max_price_per_unit: 0,
                },
            },
            tip: 0,
            paymaster_data: vec![],
            account_deployment_data: vec![],
            nonce_data_availability_mode: DataAvailabilityMode::L1,
            fee_data_availability_mode: DataAvailabilityMode::L1,
            is_query: true,
        }))
    }

    pub fn resolve_class_hash(&self) -> Result<Felt, Error> {
        match self.class_hash {
            class_hash if class_hash == ClassHash::BRAAVOS_ACCOUNT => {
                let Some(ref sigdata) = self.sigdata else {
                    return Err(Error::Execution("invalid deployment data".to_string()));
                };

                sigdata
                    .first()
                    .cloned()
                    .ok_or(Error::Execution("invalid deployment data".to_string()))
            },
            class_hash => Ok(class_hash),
        }
    }

    /// Convert the deployment parameters into a starknet function call
    pub(crate) fn as_call(&self) -> Call {
        if self.class_hash == ClassHash::BRAAVOS_ACCOUNT {
            self.as_braavos_call()
        } else {
            self.as_udc_call()
        }
    }

    fn as_udc_call(&self) -> Call {
        Call {
            to: Contract::UDC,
            selector: selector!("deployContract"),
            calldata: CalldataBuilder::new()
                .encode(&self.class_hash)
                .encode(&self.salt)
                .encode(&self.unique)
                .encode(&self.calldata)
                .build(),
        }
    }

    fn as_braavos_call(&self) -> Call {
        let sigdata = self.sigdata.clone().unwrap_or_default();
        Call {
            to: Contract::BRAAVOS_FACTORY,
            selector: selector!("deploy_braavos_account"),
            calldata: CalldataBuilder::new().encode(&self.salt).encode(&sigdata).build(),
        }
    }
}
