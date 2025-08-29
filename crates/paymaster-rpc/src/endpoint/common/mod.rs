use jsonrpsee::core::Serialize;
use paymaster_starknet::constants::Token;
use paymaster_starknet::ChainID;
use serde::Deserialize;
use starknet::core::types::Felt;

/// Deployment parameters required to deploy a contract
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeploymentParameters {
    pub address: Felt,
    pub class_hash: Felt,
    pub salt: Felt,
    pub calldata: Vec<Felt>,
    pub sigdata: Option<Vec<Felt>>,
    pub version: u8,
}

impl From<paymaster_execution::DeploymentParameters> for DeploymentParameters {
    fn from(value: paymaster_execution::DeploymentParameters) -> Self {
        Self {
            address: value.address,
            class_hash: value.class_hash,
            salt: value.salt,
            calldata: value.calldata,
            sigdata: value.sigdata,
            version: value.version,
        }
    }
}

impl From<DeploymentParameters> for paymaster_execution::DeploymentParameters {
    fn from(value: DeploymentParameters) -> Self {
        Self {
            address: value.address,
            class_hash: value.class_hash,
            salt: value.salt,
            unique: Felt::ZERO,
            calldata: value.calldata,
            sigdata: value.sigdata,
            version: value.version,
        }
    }
}

/// Execution parameters to use when executing the paymaster transaction.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "version")]
pub enum ExecutionParameters {
    #[serde(rename = "0x1")]
    V1 { fee_mode: FeeMode, time_bounds: Option<TimeBounds> },
}

impl From<paymaster_execution::ExecutionParameters> for ExecutionParameters {
    fn from(value: paymaster_execution::ExecutionParameters) -> Self {
        match value {
            paymaster_execution::ExecutionParameters::V1 { fee_mode, time_bounds } => Self::V1 {
                fee_mode: fee_mode.into(),
                time_bounds: time_bounds.map(|x| x.into()),
            },
        }
    }
}

impl From<ExecutionParameters> for paymaster_execution::ExecutionParameters {
    fn from(value: ExecutionParameters) -> Self {
        match value {
            ExecutionParameters::V1 { fee_mode, time_bounds } => Self::V1 {
                fee_mode: fee_mode.into(),
                time_bounds: time_bounds.map(|x| x.into()),
            },
        }
    }
}

impl ExecutionParameters {
    pub fn fee_mode(&self) -> FeeMode {
        match self {
            Self::V1 { fee_mode, .. } => fee_mode.clone(),
        }
    }

    pub fn gas_token(&self) -> Felt {
        match self {
            Self::V1 { fee_mode, .. } => fee_mode.gas_token(),
        }
    }

    /*
    pub fn time_bounds(&self) -> TimeBounds {
        let time_bounds = match self {
            Self::V1 { time_bounds, .. } => time_bounds.clone(),
        };

        time_bounds.unwrap_or(TimeBounds::valid_for(Duration::from_secs(3600)))
    }*/
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeBounds {
    pub execute_after: u64,
    pub execute_before: u64,
}

impl From<paymaster_execution::TimeBounds> for TimeBounds {
    fn from(value: paymaster_execution::TimeBounds) -> Self {
        Self {
            execute_after: value.execute_after,
            execute_before: value.execute_before,
        }
    }
}

impl From<TimeBounds> for paymaster_execution::TimeBounds {
    fn from(value: TimeBounds) -> Self {
        Self {
            execute_after: value.execute_after,
            execute_before: value.execute_before,
        }
    }
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
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum FeeMode {
    /// Standard fee mode when the user pays in the given token
    Default {
        gas_token: Felt,
        #[serde(default)]
        tip: TipPriority,
    },
    /// Sponsored fee mode where the provider pays for the user transaction
    Sponsored {
        #[serde(default)]
        tip: TipPriority,
    },
}

impl From<paymaster_execution::FeeMode> for FeeMode {
    fn from(value: paymaster_execution::FeeMode) -> Self {
        match value {
            paymaster_execution::FeeMode::Sponsored { tip } => Self::Sponsored { tip: tip.into() },
            paymaster_execution::FeeMode::Default { gas_token, tip } => Self::Default { gas_token, tip: tip.into() },
        }
    }
}

impl From<FeeMode> for paymaster_execution::FeeMode {
    fn from(value: FeeMode) -> Self {
        match value {
            FeeMode::Sponsored { tip } => Self::Sponsored { tip: tip.into() },
            FeeMode::Default { gas_token, tip } => Self::Default { gas_token, tip: tip.into() },
        }
    }
}

impl From<paymaster_execution::TipPriority> for TipPriority {
    fn from(value: paymaster_execution::TipPriority) -> Self {
        match value {
            paymaster_execution::TipPriority::Slow => TipPriority::Slow,
            paymaster_execution::TipPriority::Normal => TipPriority::Normal,
            paymaster_execution::TipPriority::Fast => TipPriority::Fast,
            paymaster_execution::TipPriority::Custom(x) => TipPriority::Custom(x),
        }
    }
}

impl From<TipPriority> for paymaster_execution::TipPriority {
    fn from(value: TipPriority) -> Self {
        match value {
            TipPriority::Slow => paymaster_execution::TipPriority::Slow,
            TipPriority::Normal => paymaster_execution::TipPriority::Normal,
            TipPriority::Fast => paymaster_execution::TipPriority::Fast,
            TipPriority::Custom(x) => paymaster_execution::TipPriority::Custom(x),
        }
    }
}

impl FeeMode {
    pub fn is_sponsored(&self) -> bool {
        matches!(self, Self::Sponsored { tip: _ })
    }

    /// Returns the gas token corresponding to the  [`FeeMode`]. In the case where the transaction is sponsored
    /// the gas token is set as the STRK token
    pub fn gas_token(&self) -> Felt {
        match self {
            Self::Default { gas_token, tip: _ } => *gas_token,
            Self::Sponsored { tip: _ } => Token::strk(&ChainID::Mainnet).address,
        }
    }

    pub fn tip(&self) -> TipPriority {
        match self {
            Self::Default { gas_token: _, tip } => *tip,
            Self::Sponsored { tip } => *tip,
        }
    }
}
