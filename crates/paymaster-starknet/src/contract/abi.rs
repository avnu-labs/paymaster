use std::collections::HashSet;

use starknet::core::types::{CompressedLegacyContractClass, Felt, FlattenedSierraClass, LegacyContractAbiEntry};
use starknet::core::utils::get_selector_from_name;

pub struct ABI {
    pub functions: HashSet<Felt>,
}

impl ABI {
    pub fn from_legacy(class: CompressedLegacyContractClass) -> Self {
        let mut functions = HashSet::new();
        for entry in class.abi.unwrap_or_default() {
            match entry {
                LegacyContractAbiEntry::Function(func) => {
                    functions.insert(get_selector_from_name(&func.name).unwrap());
                },
                _ => continue,
            }
        }

        Self { functions }
    }

    pub fn from_sierra(sierra: FlattenedSierraClass) -> Self {
        let mut functions = HashSet::new();
        for entry in sierra.entry_points_by_type.external {
            functions.insert(entry.selector);
        }

        Self { functions }
    }

    pub fn contains_selector(&self, selector: Felt) -> bool {
        self.functions.contains(&selector)
    }
}
