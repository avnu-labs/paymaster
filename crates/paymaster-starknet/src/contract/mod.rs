pub mod abi;

use starknet::core::types::ContractClass as StarknetContractClass;

use crate::contract::abi::ABI;

pub struct ContractClass {
    pub abi: ABI,
}

impl ContractClass {
    pub fn from_class(class: StarknetContractClass) -> Self {
        Self {
            abi: match class {
                StarknetContractClass::Legacy(class) => ABI::from_legacy(class),
                StarknetContractClass::Sierra(sierra) => ABI::from_sierra(sierra),
            },
        }
    }
}
