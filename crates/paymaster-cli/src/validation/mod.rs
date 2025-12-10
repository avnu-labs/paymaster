use paymaster_starknet::constants::Token;
use paymaster_starknet::math::denormalize_felt;
use paymaster_starknet::Client;
use starknet::core::types::Felt;

use crate::core::Error;

pub async fn assert_strk_balance(client: &Client, contract_address: Felt, amount: Felt) -> Result<(), Error> {
    let balance = client
        .fetch_balance(Token::strk(client.chain_id()).address, contract_address)
        .await
        .map_err(|e| Error::Validation(e.to_string()))?;
    if balance < amount {
        return Err(Error::Validation(format!(
            "Insufficient STRK balance: {}, needed: {}",
            denormalize_felt(balance, 18),
            denormalize_felt(amount, 18)
        )));
    }

    Ok(())
}

pub async fn assert_rebalancing_configuration(
    num_relayers: usize,
    min_relayer_balance: Felt,
    rebalancing_trigger_balance: Felt,
    initial_funding: Felt,
) -> Result<(), Error> {
    if num_relayers == 0 {
        return Err(Error::Validation("Number of relayers must be greater than 0".to_string()));
    }

    if min_relayer_balance > rebalancing_trigger_balance {
        return Err(Error::Validation(
            "Minimum relayer balance must be less than rebalancing trigger balance to ensure rebalancing is triggered before the relayer is deactivated".to_string(),
        ));
    }

    if initial_funding < rebalancing_trigger_balance * Felt::from(num_relayers) {
        return Err(Error::Validation(
            "Initial funding must be greater than num_relayers * rebalancing_trigger_balance".to_string(),
        ));
    }

    Ok(())
}
