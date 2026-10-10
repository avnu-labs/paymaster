use jsonrpsee::core::Serialize;
use paymaster_starknet::constants::Token;
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

/// Who pays the privacy pool fee of a private transaction
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "paid_by", rename_all = "snake_case")]
pub enum PoolFee {
    /// The user pays the pool fee from their private balance, in the given token
    User { token: Felt },
    /// The sponsor pays the pool fee: the user adds no fee withdrawal to their proof
    Sponsor,
}

impl From<PoolFee> for paymaster_execution::PoolFee {
    fn from(value: PoolFee) -> Self {
        match value {
            PoolFee::User { token } => Self::User { token },
            PoolFee::Sponsor => Self::Sponsor,
        }
    }
}

impl From<paymaster_execution::PoolFee> for PoolFee {
    fn from(value: paymaster_execution::PoolFee) -> Self {
        match value {
            paymaster_execution::PoolFee::User { token } => Self::User { token },
            paymaster_execution::PoolFee::Sponsor => Self::Sponsor,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum FeeMode {
    /// The user pays every fee (gas, and the pool fee on private transactions) in the given token
    Default {
        gas_token: Felt,
        #[serde(default)]
        tip: TipPriority,
    },
    /// The sponsor pays the gas fee. On private transactions `pool_fee` is required and says who pays
    /// the pool fee; on other transaction types there is no pool fee and the field must be omitted.
    Sponsored {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool_fee: Option<PoolFee>,
        #[serde(default)]
        tip: TipPriority,
    },
    /// Deprecated spelling of `sponsored` with `pool_fee: { paid_by: "user", token }`, kept so existing
    /// clients keep working. It is echoed back as sent.
    SponsoredPrivate {
        pool_fee_token: Felt,
        #[serde(default)]
        tip: TipPriority,
    },
}

impl From<paymaster_execution::FeeMode> for FeeMode {
    fn from(value: paymaster_execution::FeeMode) -> Self {
        match value {
            paymaster_execution::FeeMode::Default { gas_token, tip } => Self::Default { gas_token, tip: tip.into() },
            paymaster_execution::FeeMode::Sponsored { pool_fee, tip } => Self::Sponsored {
                pool_fee: pool_fee.map(Into::into),
                tip: tip.into(),
            },
        }
    }
}

impl From<FeeMode> for paymaster_execution::FeeMode {
    fn from(value: FeeMode) -> Self {
        match value {
            FeeMode::Default { gas_token, tip } => Self::Default { gas_token, tip: tip.into() },
            FeeMode::Sponsored { pool_fee, tip } => Self::Sponsored {
                pool_fee: pool_fee.map(Into::into),
                tip: tip.into(),
            },
            FeeMode::SponsoredPrivate { pool_fee_token, tip } => Self::Sponsored {
                pool_fee: Some(paymaster_execution::PoolFee::User { token: pool_fee_token }),
                tip: tip.into(),
            },
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
    /// Whether the sponsor pays the gas fee, which requires a valid API key
    pub fn is_sponsored(&self) -> bool {
        matches!(self, Self::Sponsored { .. } | Self::SponsoredPrivate { .. })
    }

    /// Whether the sponsor also pays the pool fee of a private transaction
    pub fn is_pool_fee_sponsored(&self) -> bool {
        matches!(
            self,
            Self::Sponsored {
                pool_fee: Some(PoolFee::Sponsor),
                ..
            }
        )
    }

    /// Returns the token charged to the user, which drives the fee estimate and the fee action.
    /// When the user pays the pool fee, it is the pool fee token. When the user pays nothing, STRK.
    pub fn gas_token(&self) -> Felt {
        match self {
            Self::Default { gas_token, .. } => *gas_token,
            Self::Sponsored {
                pool_fee: Some(PoolFee::User { token }),
                ..
            } => *token,
            Self::Sponsored { .. } => Token::STRK_ADDRESS,
            Self::SponsoredPrivate { pool_fee_token, .. } => *pool_fee_token,
        }
    }

    pub fn tip(&self) -> TipPriority {
        match self {
            Self::Default { tip, .. } => *tip,
            Self::Sponsored { tip, .. } => *tip,
            Self::SponsoredPrivate { tip, .. } => *tip,
        }
    }
}

#[cfg(test)]
mod tests {
    use paymaster_starknet::constants::Token;
    use serde_json::{json, Value};

    use super::{FeeMode, PoolFee, TipPriority};

    fn roundtrip(input: Value) -> (FeeMode, Value) {
        let fee_mode: FeeMode = serde_json::from_value(input).unwrap();
        let output = serde_json::to_value(&fee_mode).unwrap();
        (fee_mode, output)
    }

    #[test]
    fn sponsored_without_pool_fee() {
        let (fee_mode, output) = roundtrip(json!({ "mode": "sponsored", "tip": "fast" }));

        assert!(matches!(
            fee_mode,
            FeeMode::Sponsored {
                pool_fee: None,
                tip: TipPriority::Fast
            }
        ));
        assert_eq!(output, json!({ "mode": "sponsored", "tip": "fast" }));
        assert_eq!(paymaster_execution::FeeMode::from(fee_mode).gas_token(), Token::STRK_ADDRESS);
    }

    #[test]
    fn pool_fee_paid_by_user() {
        let (fee_mode, output) = roundtrip(json!({ "mode": "sponsored", "pool_fee": { "paid_by": "user", "token": Token::ETH_ADDRESS } }));

        assert!(matches!(&fee_mode, FeeMode::Sponsored { pool_fee: Some(PoolFee::User { token }), .. } if *token == Token::ETH_ADDRESS));
        assert!(!fee_mode.is_pool_fee_sponsored());
        assert_eq!(
            output,
            json!({ "mode": "sponsored", "pool_fee": { "paid_by": "user", "token": Token::ETH_ADDRESS }, "tip": "normal" })
        );
        assert_eq!(paymaster_execution::FeeMode::from(fee_mode).gas_token(), Token::ETH_ADDRESS);
    }

    #[test]
    fn pool_fee_paid_by_sponsor() {
        let (fee_mode, output) = roundtrip(json!({ "mode": "sponsored", "pool_fee": { "paid_by": "sponsor" } }));

        assert!(fee_mode.is_sponsored());
        assert!(fee_mode.is_pool_fee_sponsored());
        assert_eq!(output, json!({ "mode": "sponsored", "pool_fee": { "paid_by": "sponsor" }, "tip": "normal" }));
        assert_eq!(
            paymaster_execution::FeeMode::from(fee_mode),
            paymaster_execution::FeeMode::Sponsored {
                pool_fee: Some(paymaster_execution::PoolFee::Sponsor),
                tip: paymaster_execution::TipPriority::Normal,
            }
        );
    }

    #[test]
    fn legacy_sponsored_private_still_works_and_is_echoed_back_as_sent() {
        let (fee_mode, output) = roundtrip(json!({ "mode": "sponsored_private", "pool_fee_token": Token::ETH_ADDRESS, "tip": "slow" }));

        assert!(fee_mode.is_sponsored());
        assert!(!fee_mode.is_pool_fee_sponsored());
        assert_eq!(fee_mode.gas_token(), Token::ETH_ADDRESS);
        assert_eq!(output, json!({ "mode": "sponsored_private", "pool_fee_token": Token::ETH_ADDRESS, "tip": "slow" }));

        assert_eq!(
            paymaster_execution::FeeMode::from(fee_mode),
            paymaster_execution::FeeMode::Sponsored {
                pool_fee: Some(paymaster_execution::PoolFee::User { token: Token::ETH_ADDRESS }),
                tip: paymaster_execution::TipPriority::Slow,
            }
        );
    }
}
