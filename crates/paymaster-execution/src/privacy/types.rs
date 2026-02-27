use rand::Rng;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

pub fn generate_non_zero_random_felt() -> Felt {
    loop {
        let value: u128 = rand::rng().random();
        if value != 0 {
            return Felt::from(value);
        }
    }
}

/// Privacy pool client action, matching the SDK's `ClientAction` tagged union.
/// Format: `{ "type": "<Variant>", "input": { ... } }`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "input")]
pub enum ClientAction {
    SetViewingKey(SetViewingKeyInput),
    OpenChannel(OpenChannelInput),
    OpenSubchannel(OpenSubchannelInput),
    CreateEncNote(CreateEncNoteInput),
    CreateOpenNote(CreateOpenNoteInput),
    Deposit(DepositInput),
    UseNote(UseNoteInput),
    Withdraw(WithdrawInput),
    InvokeExternal(InvokeExternalInput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetViewingKeyInput {
    pub random: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelInput {
    pub recipient_addr: Felt,
    pub recipient_public_key: Felt,
    pub index: u32,
    pub random: Felt,
    pub salt: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSubchannelInput {
    pub recipient_addr: Felt,
    pub recipient_public_key: Felt,
    pub channel_key: Felt,
    pub index: u32,
    pub token: Felt,
    pub salt: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEncNoteInput {
    pub recipient_addr: Felt,
    pub recipient_public_key: Felt,
    pub token: Felt,
    pub amount: Felt,
    pub index: u32,
    pub salt: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOpenNoteInput {
    pub recipient_addr: Felt,
    pub recipient_public_key: Felt,
    pub token: Felt,
    pub index: u32,
    pub depositor: Felt,
    pub random: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInput {
    pub token: Felt,
    pub amount: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseNoteInput {
    pub channel_key: Felt,
    pub token: Felt,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawInput {
    pub to_addr: Felt,
    pub token: Felt,
    pub amount: Felt,
    pub random: Felt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeExternalInput {
    pub contract_address: Felt,
    pub calldata: serde_json::Value,
}

/// Insert a Withdraw action before the first `InvokeExternal` (phase ordering).
/// If no `InvokeExternal` is present, append at the end.
pub fn insert_withdraw_action(actions: &mut Vec<ClientAction>, withdraw: ClientAction) {
    let pos = actions.iter().position(|a| matches!(a, ClientAction::InvokeExternal(_)));
    match pos {
        Some(idx) => actions.insert(idx, withdraw),
        None => actions.push(withdraw),
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::*;

    fn a_withdraw() -> ClientAction {
        ClientAction::Withdraw(WithdrawInput {
            to_addr: Felt::from(0x1u64),
            token: Felt::from(0x2u64),
            amount: Felt::from(100u64),
            random: Felt::from(0x42u64),
        })
    }

    fn a_deposit() -> ClientAction {
        ClientAction::Deposit(DepositInput {
            token: Felt::from(0x1u64),
            amount: Felt::from(50u64),
        })
    }

    fn an_invoke_external() -> ClientAction {
        ClientAction::InvokeExternal(InvokeExternalInput {
            contract_address: Felt::from(0xAAu64),
            calldata: serde_json::Value::Array(vec![]),
        })
    }

    mod insert_withdraw_action {
        use super::*;

        #[test]
        fn should_insert_before_invoke_external_when_present() {
            // Given
            let mut actions = vec![a_deposit(), an_invoke_external()];

            // When
            insert_withdraw_action(&mut actions, a_withdraw());

            // Then
            assert_eq!(actions.len(), 3);
            assert!(matches!(actions[0], ClientAction::Deposit(_)));
            assert!(matches!(actions[1], ClientAction::Withdraw(_)));
            assert!(matches!(actions[2], ClientAction::InvokeExternal(_)));
        }

        #[test]
        fn should_append_at_end_when_no_invoke_external() {
            // Given
            let mut actions = vec![a_deposit()];

            // When
            insert_withdraw_action(&mut actions, a_withdraw());

            // Then
            assert_eq!(actions.len(), 2);
            assert!(matches!(actions[0], ClientAction::Deposit(_)));
            assert!(matches!(actions[1], ClientAction::Withdraw(_)));
        }

        #[test]
        fn should_insert_before_first_invoke_external_when_multiple() {
            // Given
            let mut actions = vec![a_deposit(), an_invoke_external(), an_invoke_external()];

            // When
            insert_withdraw_action(&mut actions, a_withdraw());

            // Then
            assert_eq!(actions.len(), 4);
            assert!(matches!(actions[0], ClientAction::Deposit(_)));
            assert!(matches!(actions[1], ClientAction::Withdraw(_)));
            assert!(matches!(actions[2], ClientAction::InvokeExternal(_)));
            assert!(matches!(actions[3], ClientAction::InvokeExternal(_)));
        }

        #[test]
        fn should_be_only_action_when_actions_empty() {
            // Given
            let mut actions = vec![];

            // When
            insert_withdraw_action(&mut actions, a_withdraw());

            // Then
            assert_eq!(actions.len(), 1);
            assert!(matches!(actions[0], ClientAction::Withdraw(_)));
        }
    }

    mod generate_non_zero_random_felt {
        use super::*;

        #[test]
        fn should_always_produce_non_zero_value() {
            // When / Then
            for _ in 0..100 {
                assert_ne!(generate_non_zero_random_felt(), Felt::ZERO);
            }
        }
    }
}
