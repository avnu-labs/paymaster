use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{Call, Felt, TypedData};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildTransactionRequest {
    pub transaction: TransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransactionParameters {
    Deploy { deployment: DeploymentParameters },
    Invoke { invoke: InvokeParameters },
    DeployAndInvoke { deployment: DeploymentParameters, invoke: InvokeParameters },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InvokeParameters {
    pub user_address: Felt,
    pub calls: Vec<Call>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeploymentParameters {
    pub address: Felt,
    pub class_hash: Felt,
    pub salt: Felt,
    pub calldata: Vec<Felt>,
    pub sigdata: Option<Vec<Felt>>,
    pub version: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "version")]
pub enum ExecutionParameters {
    #[serde(rename = "0x1")]
    V1 { fee_mode: FeeMode, time_bounds: Option<TimeBounds> },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum FeeMode {
    Default {
        gas_token: Felt,
        #[serde(default)]
        tip: TipPriority,
    },
    Sponsored {
        #[serde(default)]
        tip: TipPriority,
    },
}

#[derive(Serialize, Deserialize, Copy, Default, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TipPriority {
    Slow,
    #[default]
    Normal,
    Fast,
    Custom(u64),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeBounds {
    pub execute_after: u64,
    pub execute_before: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildTransactionResponse {
    Deploy(DeployTransaction),
    Invoke(InvokeTransaction),
    DeployAndInvoke(DeployAndInvokeTransaction),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeployTransaction {
    pub deployment: DeploymentParameters,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InvokeTransaction {
    pub typed_data: TypedData,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeployAndInvokeTransaction {
    pub deployment: DeploymentParameters,
    pub typed_data: TypedData,
    pub parameters: ExecutionParameters,
    pub fee: FeeEstimate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FeeEstimate {
    pub gas_token_price_in_strk: Felt,
    pub estimated_fee_in_strk: Felt,
    pub estimated_fee_in_gas_token: Felt,
    pub suggested_max_fee_in_strk: Felt,
    pub suggested_max_fee_in_gas_token: Felt,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteRequest {
    pub transaction: ExecutableTransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutableTransactionParameters {
    Deploy {
        deployment: DeploymentParameters,
    },
    Invoke {
        invoke: ExecutableInvokeParameters,
    },
    DeployAndInvoke {
        deployment: DeploymentParameters,
        invoke: ExecutableInvokeParameters,
    },
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutableInvokeParameters {
    #[serde_as(as = "UfeHex")]
    pub user_address: Felt,
    pub typed_data: TypedData,
    #[serde_as(as = "Vec<UfeHex>")]
    pub signature: Vec<Felt>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteResponse {
    #[serde_as(as = "UfeHex")]
    pub transaction_hash: Felt,
    #[serde_as(as = "UfeHex")]
    pub tracking_id: Felt,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteDirectRequest {
    pub transaction: ExecuteDirectTransactionParameters,
    pub parameters: ExecutionParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecuteDirectTransactionParameters {
    Invoke { invoke: DirectInvokeParameters },
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectInvokeParameters {
    #[serde_as(as = "UfeHex")]
    pub user_address: Felt,
    pub execute_from_outside_call: Call,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteDirectResponse {
    #[serde_as(as = "UfeHex")]
    pub transaction_hash: Felt,
    #[serde_as(as = "UfeHex")]
    pub tracking_id: Felt,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct TokenPrice {
    pub token_address: Felt,
    pub decimals: i64,
    pub price_in_strk: Felt,
}
