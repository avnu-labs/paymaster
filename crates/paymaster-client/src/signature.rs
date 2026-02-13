use starknet::core::types::{Felt, TypedData};
use starknet::signers::Signer;
use crate::Error;

pub async fn sign_typed_data<S>(typed_data: &TypedData, address: Felt, signer: &S) -> Result<Vec<Felt>, Error>
where
    S: Signer + Send + Sync,
{
    let message_hash = typed_data
        .message_hash(address)
        .map_err(|e| Error::Signing(format!("failed to compute message hash: {e}")))?;

    let sig = signer
        .sign_hash(&message_hash)
        .await
        .map_err(|e| Error::Signing(e.to_string()))?;

    Ok(vec![sig.r, sig.s])
}