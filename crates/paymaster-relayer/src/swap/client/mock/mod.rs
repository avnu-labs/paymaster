use std::fmt::Debug;

use async_trait::async_trait;
use paymaster_common::service::Error as ServiceError;
use starknet::core::types::{Call, Felt};

#[async_trait]
pub trait MockSwapClient: 'static + Send + Sync + Debug {
    fn new() -> Self
    where
        Self: Sized;

    async fn swap(
        &self,
        _sell_token: Felt,
        _buy_token: Felt,
        _sell_amount: Felt,
        _taker_address: Felt,
        _slippage: f64,
        _max_price_impact: f64,
        _min_usd_sell_amount: f64,
    ) -> Result<(Vec<Call>, Felt), ServiceError> {
        unimplemented!()
    }
}

/// Simple mock implementation for testing
#[derive(Debug, Clone)]
pub struct MockSimpleSwap;

impl MockSimpleSwap {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MockSwapClient for MockSimpleSwap {
    fn new() -> Self {
        Self
    }

    async fn swap(
        &self,
        _sell_token: Felt,
        _buy_token: Felt,
        sell_amount: Felt,
        _taker_address: Felt,
        _slippage: f64,
        _max_price_impact: f64,
        _min_usd_sell_amount: f64,
    ) -> Result<(Vec<Call>, Felt), ServiceError> {
        // Return empty calls and the same amount as "received" for testing
        Ok((vec![], sell_amount))
    }
}
