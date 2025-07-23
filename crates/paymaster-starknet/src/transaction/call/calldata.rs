use starknet::core::types::{Call, Felt};

pub trait AsCalldata {
    fn encode(&self) -> Vec<Felt>;
}

impl AsCalldata for Felt {
    fn encode(&self) -> Vec<Felt> {
        vec![*self]
    }
}

impl AsCalldata for Call {
    fn encode(&self) -> Vec<Felt> {
        CalldataBuilder::new()
            .encode(&self.to)
            .encode(&self.selector)
            .encode(&self.calldata)
            .build()
    }
}

impl<T: AsCalldata> AsCalldata for Vec<T> {
    fn encode(&self) -> Vec<Felt> {
        let mut calldata = vec![];
        calldata.push(Felt::from(self.len()));
        calldata.extend(self.iter().flat_map(|x| x.encode()));

        calldata
    }
}

impl<T: AsCalldata> AsCalldata for &[T] {
    fn encode(&self) -> Vec<Felt> {
        let mut calldata = vec![];
        calldata.push(Felt::from(self.len()));
        calldata.extend(self.iter().flat_map(|x| x.encode()));

        calldata
    }
}

#[derive(Default)]
pub struct CalldataBuilder {
    calldata: Vec<Felt>,
}

impl CalldataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encode<T: AsCalldata>(mut self, value: &T) -> Self {
        self.calldata.extend(value.encode());
        self
    }

    pub fn build(self) -> Vec<Felt> {
        self.calldata
    }
}
