use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use crate::transaction::{AsCalldata, CalldataBuilder};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeBounds {
    pub execute_after: u64,
    pub execute_before: u64,
}

impl TimeBounds {
    pub fn is_valid(&self) -> bool {
        let now = Utc::now().timestamp() as u64;
        if self.execute_after >= self.execute_before {
            return false;
        }

        self.execute_after <= now && now < self.execute_before
    }

    pub fn valid_for(validity: Duration) -> Self {
        Self {
            execute_after: 1,
            execute_before: (Utc::now() + validity).timestamp() as u64,
        }
    }
}

impl AsCalldata for TimeBounds {
    fn encode(&self) -> Vec<Felt> {
        CalldataBuilder::new()
            .encode(&Felt::from(self.execute_after))
            .encode(&Felt::from(self.execute_before))
            .build()
    }
}
