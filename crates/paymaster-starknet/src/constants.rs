use starknet::core::types::Felt;
use starknet::macros::felt;

use crate::ChainID;

/// Forwarder class hashes for different networks
pub struct ClassHash;

impl ClassHash {
    pub const ARGENT_ACCOUNT: Felt = felt!("0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f");
    pub const BRAAVOS_ACCOUNT: Felt = Felt::from_raw([185241609756504736, 2778776175894593663, 3570588520378882234, 1478234888750183556]);
    pub const FORWARDER: Felt = felt!("0x054e57545b42b9e06a372026d20238d192bfc5378110670cb0ddb8b295014af9");
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
    pub const fn eth(chain_id: &ChainID) -> Token {
        match chain_id {
            ChainID::Sepolia => Token {
                symbol: "ETH",
                decimals: 18,
                address: felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
            },
            ChainID::Mainnet => Token {
                symbol: "ETH",
                decimals: 18,
                address: felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
            },
        }
    }

    pub const fn strk(chain_id: &ChainID) -> Token {
        match chain_id {
            ChainID::Sepolia => Token {
                symbol: "STRK",
                decimals: 18,
                address: felt!("0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d"),
            },
            ChainID::Mainnet => Token {
                symbol: "STRK",
                decimals: 18,
                address: felt!("0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d"),
            },
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
        }
    }
}

pub struct Endpoint;

impl Endpoint {
    pub const fn default_rpc_url(chain_id: &ChainID) -> &'static str {
        match chain_id {
            ChainID::Sepolia => "https://starknet-sepolia.public.blastapi.io/rpc/v0_8",
            ChainID::Mainnet => "https://starknet-mainnet.public.blastapi.io/rpc/v0_8",
        }
    }

    pub const fn default_price_url(chain_id: &ChainID) -> &'static str {
        match chain_id {
            ChainID::Sepolia => "https://sepolia.impulse.avnu.fi/v2/tokens/prices",
            ChainID::Mainnet => "https://starknet.impulse.avnu.fi/v2/tokens/prices",
        }
    }

    pub const fn default_swap_url(chain_id: &ChainID) -> &'static str {
        match chain_id {
            ChainID::Sepolia => "https://sepolia.api.avnu.fi/swap/v2",
            ChainID::Mainnet => "https://starknet.api.avnu.fi/swap/v2",
        }
    }
}
