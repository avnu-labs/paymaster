use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("internal error {0}")]
    Internal(String),

    #[error("privacy requires sponsoring")]
    PrivacyRequiresSponsoring,

    #[error("missing fee TransferTo action for an accepted recipient")]
    MissingFeeTransferTo,

    #[error("failed to parse ServerActions from calldata: {0}")]
    CalldataParsing(String),

    #[error("privacy pool address is not whitelisted")]
    PrivacyPoolNotWhitelisted,

    #[error("invalid apply_actions selector")]
    InvalidApplyActionsSelector,

    #[error("invalid nonce")]
    InvalidNonce,

    #[error("invalid version")]
    InvalidVersion,

    #[error("invalid time bounds")]
    InvalidTimeBound,

    #[error("no calls specified in invoke")]
    NoCalls,

    #[error("invalid typed data")]
    InvalidTypedData,

    #[error("max amount of gas token too low. Expected at least {0}")]
    MaxAmountTooLow(String),

    #[error("pool fee amount too low. Expected at least {0}")]
    PoolFeeTooLow(String),

    #[error("execution error {0}")]
    Execution(String),
}

impl From<paymaster_starknet::Error> for Error {
    fn from(value: paymaster_starknet::Error) -> Self {
        match value {
            paymaster_starknet::Error::InvalidNonce(_) => Self::InvalidVersion,
            e => Self::Execution(e.to_string()),
        }
    }
}

impl From<paymaster_relayer::Error> for Error {
    fn from(value: paymaster_relayer::Error) -> Self {
        match value {
            paymaster_relayer::Error::InvalidNonce => Self::InvalidNonce,
            e => Self::Execution(e.to_string()),
        }
    }
}

impl From<paymaster_prices::Error> for Error {
    fn from(value: paymaster_prices::Error) -> Self {
        Self::Execution(value.to_string())
    }
}
