use std::fmt::Debug;

use async_trait::async_trait;
use starknet::core::types::Felt;

use crate::{Error, TokenPrice};

#[async_trait]
pub trait MockPriceOracle: 'static + Send + Sync + Debug {
    fn new() -> Self
    where
        Self: Sized;

    async fn fetch_token(&self, _address: Felt) -> Result<TokenPrice, Error> {
        unimplemented!()
    }
}
