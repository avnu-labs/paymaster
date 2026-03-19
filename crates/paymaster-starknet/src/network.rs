use std::str::FromStr;

use serde::{Deserialize, Serialize};
use starknet::core::chain_id::{MAINNET, SEPOLIA};
use starknet::core::types::Felt;

use crate::Error;

pub const SN_INTEGRATION_SEPOLIA: Felt = Felt::from_raw([356451801641654316, 18443812999431337335, 7869628190608881772, 122696121091289693]);

/// Represent the chain id which is either Sepolia or Mainnet
#[derive(Debug, Clone, Copy, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainID {
    Sepolia,
    Integration,
    Mainnet,
}

impl ChainID {
    /// Convert the identifier representation to a ChainID
    /// - SN_SEPOLIA -> ChainID::Sepolia
    /// - SN_MAINNET -> ChainID::Mainnet
    ///
    /// If the conversion fail, return an error
    pub fn from_identifier(s: &str) -> Result<Self, Error> {
        match s {
            "SN_SEPOLIA" => Ok(Self::Sepolia),
            "SN_MAIN" => Ok(Self::Mainnet),
            "SN_INTEGRATION_SEPOLIA" => Ok(Self::Integration),
            _ => Err(Error::TypedDataDecoding(format!("invalid domain {}", s))),
        }
    }

    /// Convert the ChainID to the identifier representation
    /// - ChainID:Sepolia -> "SN_SEPOLIA"
    /// - ChainID::Mainnet -> "SN_MAINNET"
    pub fn as_identifier(&self) -> String {
        String::from_str(match self {
            Self::Sepolia => "SN_SEPOLIA",
            Self::Mainnet => "SN_MAIN",
            Self::Integration => "SN_INTEGRATION_SEPOLIA",
        })
        .unwrap()
    }

    /// Convert a valid chain-id string into a ChainID
    /// - sepolia -> ChainID::Sepolia
    /// - mainnet -> ChainID::Mainnet
    ///
    /// If the conversion fail, return an error
    pub fn from_string(s: &str) -> Result<Self, Error> {
        match s {
            "integration" => Ok(Self::Integration),
            "Integration" => Ok(Self::Integration),
            "SN_INTEGRATION_SEPOLIA" => Ok(Self::Integration),
            "sepolia" => Ok(Self::Sepolia),
            "SEPOLIA" => Ok(Self::Sepolia),
            "Sepolia" => Ok(Self::Sepolia),
            "SN_SEPOLIA" => Ok(Self::Sepolia),
            "mainnet" => Ok(Self::Mainnet),
            "Mainnet" => Ok(Self::Mainnet),
            "SN_MAINNET" => Ok(Self::Mainnet),
            "main" => Ok(Self::Mainnet),
            "MAIN" => Ok(Self::Mainnet),
            _ => Err(Error::TypedDataDecoding(format!("invalid domain {}", s))),
        }
    }

    /// Convert a Felt into a ChainID
    ///
    /// If the conversion fail, return an error
    pub fn from_felt(value: Felt) -> Result<Self, Error> {
        if value == SN_INTEGRATION_SEPOLIA {
            Ok(Self::Sepolia)
        } else if value == SEPOLIA {
            Ok(Self::Sepolia)
        } else if value == MAINNET {
            Ok(Self::Mainnet)
        } else {
            Err(Error::TypedDataDecoding(format!("invalid domain {}", value.to_hex_string())))
        }
    }

    pub fn as_felt(&self) -> Felt {
        match self {
            Self::Integration => SN_INTEGRATION_SEPOLIA,
            Self::Sepolia => SEPOLIA,
            Self::Mainnet => MAINNET,
        }
    }
}
