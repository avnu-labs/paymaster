use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

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