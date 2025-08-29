use paymaster_starknet::transaction::Calls;
use paymaster_starknet::StarknetAccount;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{Call, Felt};

pub struct InvokeTransaction {
    pub to: Felt,
    pub selector: Felt,
    pub calldata: Vec<Felt>,
}

impl InvokeTransaction {
    pub async fn execute(self, account: &StarknetAccount) {
        let calls = Calls::new(vec![self.as_call()]);
        let estimated_calls = calls.estimate(account, None).await.unwrap();

        let nonce = account.get_nonce().await.unwrap();
        estimated_calls.execute(account, nonce).await.unwrap();
    }

    pub fn as_call(&self) -> Call {
        Call {
            to: self.to,
            selector: self.selector,
            calldata: self.calldata.clone(),
        }
    }
}
