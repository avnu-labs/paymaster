mod build;
pub use build::{
    EstimatedPrivateTransaction, EstimatedTransaction, InvokeParameters, PrivateInvokeUserCalls, PrivateTransaction, Transaction, TransactionParameters,
    VersionedTransaction,
};

mod deploy;
pub use deploy::DeploymentParameters;

mod execute;
pub use execute::{
    EstimatedExecutableTransaction, ExecutableApplyActionParameters, ExecutableDirectInvokeParameters, ExecutableInvokeParameters, ExecutableTransaction,
    ExecutableTransactionParameters,
};

mod fee;
pub use fee::{FeeAction, FeeEstimate, ValidationGasOverhead};
use jsonrpsee::core::Serialize;
use paymaster_starknet::constants::Token;
pub use paymaster_starknet::transaction::TimeBounds;
use serde::Deserialize;
use starknet::core::types::Felt;
use std::time::Duration;

/// Execution parameters to use when executing the paymaster transaction.
#[derive(Debug, Clone)]
pub enum ExecutionParameters {
    V1 { fee_mode: FeeMode, time_bounds: Option<TimeBounds> },
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

    pub fn tip(&self) -> TipPriority {
        match self {
            Self::V1 { fee_mode, .. } => fee_mode.tip(),
        }
    }

    pub fn time_bounds(&self) -> TimeBounds {
        let time_bounds = match self {
            Self::V1 { time_bounds, .. } => time_bounds.clone(),
        };

        time_bounds.unwrap_or(TimeBounds::valid_for(Duration::from_secs(3600)))
    }
}

#[derive(Serialize, Deserialize, Copy, Debug, Clone, PartialEq, Eq)]
pub enum TipPriority {
    Slow,
    Normal,
    Fast,
    Custom(u64),
}

/// Who pays the privacy pool fee of a private transaction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolFee {
    /// The user pays the pool fee from their private balance, in the given token
    User { token: Felt },
    /// The sponsor pays the pool fee: the user adds no fee withdrawal to their proof
    Sponsor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeeMode {
    /// The user pays every fee (gas, and the pool fee on private transactions) in the given token
    Default { gas_token: Felt, tip: TipPriority },
    /// The sponsor pays the gas fee. `pool_fee` says who pays the pool fee of a private transaction, and
    /// is `None` when the transaction has no pool fee (non-private transaction types).
    Sponsored { pool_fee: Option<PoolFee>, tip: TipPriority },
}

impl FeeMode {
    /// Whether the sponsor pays the gas fee, which requires a valid API key
    pub fn is_sponsored(&self) -> bool {
        matches!(self, Self::Sponsored { .. })
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
        }
    }

    pub fn tip(&self) -> TipPriority {
        match self {
            Self::Default { tip, .. } => *tip,
            Self::Sponsored { tip, .. } => *tip,
        }
    }
}

#[cfg(test)]
mod tests {
    use paymaster_starknet::constants::Token;

    use super::{FeeMode, PoolFee, TipPriority};

    #[test]
    fn sponsored_without_pool_fee_charges_nothing_to_the_user() {
        let fee_mode = FeeMode::Sponsored {
            pool_fee: None,
            tip: TipPriority::Normal,
        };

        assert!(fee_mode.is_sponsored());
        assert!(!fee_mode.is_pool_fee_sponsored());
        assert_eq!(fee_mode.gas_token(), Token::STRK_ADDRESS);
    }

    #[test]
    fn pool_fee_paid_by_user_charges_that_token() {
        let fee_mode = FeeMode::Sponsored {
            pool_fee: Some(PoolFee::User { token: Token::ETH_ADDRESS }),
            tip: TipPriority::Normal,
        };

        assert!(fee_mode.is_sponsored());
        assert!(!fee_mode.is_pool_fee_sponsored());
        assert_eq!(fee_mode.gas_token(), Token::ETH_ADDRESS);
    }

    #[test]
    fn pool_fee_paid_by_sponsor_charges_nothing_to_the_user() {
        let fee_mode = FeeMode::Sponsored {
            pool_fee: Some(PoolFee::Sponsor),
            tip: TipPriority::Normal,
        };

        assert!(fee_mode.is_sponsored());
        assert!(fee_mode.is_pool_fee_sponsored());
        assert_eq!(fee_mode.gas_token(), Token::STRK_ADDRESS);
    }
}
