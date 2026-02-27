use starknet::accounts::ExecutionEncoding;
use starknet::core::types::{BlockId, BlockTag, Call, Felt, FunctionCall};
use starknet::macros::{felt, selector};
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};

use crate::client::StarknetClient;
use crate::transaction::{Calls, EstimatedCalls, PrivateProofData};
use crate::{Error, StarknetAccount};

const PATHFINDER_RPC: &str = "http://34.170.239.64:9545/rpc/v0_10";
const POOL_ADDRESS: Felt = felt!("0x7f80063a08907696aa5e891498c66a60dc8a46544c59742000d976a3bedd7ca");
const ADMIN_ADDRESS: Felt = felt!("0x048baf3ed1f0a03840186bd95063f63824d93bafd456439bfe667533437d9c91");
const ADMIN_PRIVATE_KEY: Felt = felt!("0x7021e74994902199b1fa41785e15ade56f3ba5d208818b620a3741e68845d94");
const STRK_TOKEN_ADDRESS: Felt = felt!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

fn devnet_client() -> StarknetClient {
    StarknetClient::new(PATHFINDER_RPC, 30)
}

async fn devnet_account() -> StarknetAccount {
    let client = devnet_client();
    let chain_id = client.chain_id().await.unwrap();
    let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(ADMIN_PRIVATE_KEY));
    let mut account = StarknetAccount::new(client, signer, ADMIN_ADDRESS, chain_id, ExecutionEncoding::New);
    account.set_block_id(BlockId::Tag(BlockTag::Latest));
    account
}

mod read_pool_fee {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn should_return_non_zero_fee_when_pool_deployed() {
        // Given
        let client = devnet_client();
        let call = FunctionCall {
            contract_address: POOL_ADDRESS,
            entry_point_selector: selector!("get_fee_amount"),
            calldata: vec![],
        };

        // When
        let result = client.call(&call, BlockId::Tag(BlockTag::Latest)).await.unwrap();

        // Then
        let fee: u128 = result[0].try_into().unwrap();
        assert!(fee > 0, "Pool fee should be non-zero, got {}", fee);
    }
}

mod estimate_with_proof {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn should_estimate_fee_when_privacy_tx_with_mock_proof() {
        // Given
        let account = devnet_account().await;

        let pool_fee = {
            let client = devnet_client();
            let call = FunctionCall {
                contract_address: POOL_ADDRESS,
                entry_point_selector: selector!("get_fee_amount"),
                calldata: vec![],
            };
            let result = client.call(&call, BlockId::Tag(BlockTag::Latest)).await.unwrap();
            let fee: u128 = result[0].try_into().unwrap();
            fee
        };

        let approve_call = Call {
            to: STRK_TOKEN_ADDRESS,
            selector: selector!("approve"),
            calldata: vec![POOL_ADDRESS, Felt::from(pool_fee), Felt::ZERO],
        };
        let apply_actions_call = Call {
            to: POOL_ADDRESS,
            selector: selector!("apply_actions"),
            calldata: vec![Felt::ZERO], // empty actions span (length = 0)
        };
        let calls = Calls::new(vec![approve_call, apply_actions_call]);

        let proof_data = PrivateProofData {
            proof: vec![0u64],
            proof_facts: vec![Felt::ONE; 9],
        };

        // When
        let result: Result<EstimatedCalls, Error> = calls.estimate_with_proof(&account, None, &proof_data).await;

        // Then — estimation should succeed (SkipValidate bypasses proof validation)
        assert!(result.is_ok(), "Estimation failed: {:?}", result.err());
    }
}
