use paymaster_starknet::transaction::CalldataBuilder;
use paymaster_starknet::StarknetAccount;
use starknet::accounts::Account;
use starknet::core::types::{Call, Felt, InvokeTransactionResult};
use starknet::macros::selector;

use crate::core::Error;

pub struct Transfer {
    pub token: Felt,
    pub recipient: Felt,
    pub amount: Felt,
}

impl Transfer {
    pub async fn execute(&self, account: &StarknetAccount) -> Result<InvokeTransactionResult, Error> {
        let call = Call {
            to: self.token,
            selector: selector!("transfer"),
            calldata: CalldataBuilder::new()
                .encode(&self.recipient)
                .encode(&self.amount)
                .encode(&Felt::ZERO)
                .build(),
        };

        let result = account.execute_v3(vec![call]).send().await.unwrap();
        Ok(result)
    }

    pub fn as_call(&self) -> Call {
        Call {
            to: self.token,
            selector: selector!("transfer"),
            calldata: CalldataBuilder::new()
                .encode(&self.recipient)
                .encode(&self.amount)
                .encode(&Felt::ZERO)
                .build(),
        }
    }
}
