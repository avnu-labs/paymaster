use starknet::core::types::Felt;
use starknet::macros::felt;

use crate::ChainID;

/// Forwarder class hashes for different networks
pub struct ClassHash;

impl ClassHash {
    pub const ARGENT_ACCOUNT: Felt = felt!("0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f");
    pub const BRAAVOS_ACCOUNT: Felt = Felt::from_raw([185241609756504736, 2778776175894593663, 3570588520378882234, 1478234888750183556]);
    // TODO: revert
    pub const FORWARDER: Felt = felt!("0x5a948a1ac99bef70779ed05ae2a8c0c27bec4d6b1d92350d48f9644b7a2edab");
}

/// Contract addresses for different networks
pub struct Contract;

impl Contract {
    pub const BRAAVOS_FACTORY: Felt = felt!("0x03d94f65ebc7552eb517ddb374250a9525b605f25f4e41ded6e7d7381ff1c2e8");
    pub const UDC: Felt = felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
}

pub struct Token {
    pub symbol: &'static str,
    pub decimals: u32,
    pub address: Felt,
}

impl Token {
    pub const ETH_ADDRESS: Felt = felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
    // TODO: revert
    pub const STRK_ADDRESS: Felt = felt!("0x70a5da4f557b77a9c54546e4bcc900806e28793d8e3eaaa207428d2387249b7");

    pub const fn eth() -> Token {
        Token {
            symbol: "ETH",
            decimals: 18,
            address: Self::ETH_ADDRESS,
        }
    }

    pub const fn strk() -> Token {
        Token {
            symbol: "STRK",
            decimals: 18,
            address: Self::STRK_ADDRESS,
        }
    }

    pub const fn usdc(chain_id: &ChainID) -> Token {
        match chain_id {
            ChainID::Sepolia => Token {
                symbol: "USDC",
                decimals: 6,
                address: felt!("0x53b40a647cedfca6ca84f542a0fe36736031905a9639a7f19a3c1e66bfd5080"),
            },
            ChainID::Mainnet => Token {
                symbol: "USDC",
                decimals: 6,
                address: felt!("0x53c91253bc9682c04929ca02ed00b3e423f6710d2ee7e0d5ebb06f3ecf368a8"),
            },
            ChainID::Integration => Token {
                symbol: "TestToken1",
                decimals: 6,
                address: felt!("0x7b19e89252b1ee5d7ff07a0e0e278b16b058f322053f799469b969e31b82969"),
            },
        }
    }
}
