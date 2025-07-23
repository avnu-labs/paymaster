mod build;
pub use build::{EstimatedTransaction, InvokeParameters, Transaction, TransactionParameters, VersionedTransaction};

mod deploy;
pub use deploy::DeploymentParameters;

mod execute;
pub use execute::{EstimatedExecutableTransaction, ExecutableInvokeParameters, ExecutableTransaction, ExecutableTransactionParameters};

mod fee;
use std::time::Duration;

pub use fee::{FeeEstimate, ValidationGasOverhead};
use paymaster_starknet::constants::Token;
pub use paymaster_starknet::transaction::TimeBounds;
use paymaster_starknet::ChainID;
use starknet::core::types::Felt;

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

    pub fn time_bounds(&self) -> TimeBounds {
        let time_bounds = match self {
            Self::V1 { time_bounds, .. } => time_bounds.clone(),
        };

        time_bounds.unwrap_or(TimeBounds::valid_for(Duration::from_secs(3600)))
    }
}

#[derive(Debug, Clone)]
pub enum FeeMode {
    /// Standard fee mode when the user pays in the given token
    Default { gas_token: Felt },
    /// Sponsored fee mode where the provider pays for the user transaction
    Sponsored,
}

impl FeeMode {
    pub fn is_sponsored(&self) -> bool {
        matches!(self, Self::Sponsored)
    }

    /// Returns the gas token corresponding to the  [`FeeMode`]. In the case where the transaction is sponsored
    /// the gas token is set as the STRK token
    pub fn gas_token(&self) -> Felt {
        match self {
            Self::Default { gas_token } => *gas_token,
            Self::Sponsored => Token::strk(&ChainID::Mainnet).address,
        }
    }
}
