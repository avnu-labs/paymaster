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

#[derive(Serialize, Deserialize, Copy, Debug, Clone)]
pub enum TipPriority {
    Slow,
    Normal,
    Fast,
    Custom(u64),
}

#[derive(Debug, Clone)]
pub enum FeeMode {
    /// Standard fee mode when the user pays in the given token
    Default { gas_token: Felt, tip: TipPriority },
    /// Sponsored fee mode where the provider pays for the user transaction
    Sponsored { tip: TipPriority },
    /// Sponsored fee mode for private transactions where the user chooses the pool fee token
    SponsoredPrivate { pool_fee_token: Felt, tip: TipPriority },
}

impl FeeMode {
    pub fn is_sponsored(&self) -> bool {
        matches!(self, Self::Sponsored { .. } | Self::SponsoredPrivate { .. })
    }

    /// Returns the gas token corresponding to the [`FeeMode`].
    /// For sponsored transactions the gas token is STRK.
    /// For sponsored_private transactions the gas token is the user-chosen pool fee token.
    pub fn gas_token(&self) -> Felt {
        match self {
            Self::Default { gas_token, .. } => *gas_token,
            Self::Sponsored { .. } => Token::STRK_ADDRESS,
            Self::SponsoredPrivate { pool_fee_token, .. } => *pool_fee_token,
        }
    }

    pub fn tip(&self) -> TipPriority {
        match self {
            Self::Default { tip, .. } => *tip,
            Self::Sponsored { tip } => *tip,
            Self::SponsoredPrivate { tip, .. } => *tip,
        }
    }

    pub fn is_sponsored_private(&self) -> bool {
        matches!(self, Self::SponsoredPrivate { .. })
    }
}
