use std::hash::RandomState;

use indexmap::IndexMap;
use starknet::core::types::typed_data::{ArrayValue, ObjectValue, Value};
use starknet::core::types::Felt;

pub trait EncodeAsTypedValue {
    fn encode(&self) -> Value;
}

impl EncodeAsTypedValue for Value {
    fn encode(&self) -> Value {
        self.clone()
    }
}

impl EncodeAsTypedValue for Felt {
    fn encode(&self) -> Value {
        Value::String(self.to_hex_string())
    }
}

impl EncodeAsTypedValue for String {
    fn encode(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl EncodeAsTypedValue for usize {
    fn encode(&self) -> Value {
        Value::UnsignedInteger(*self as u128)
    }
}

impl<T: EncodeAsTypedValue> EncodeAsTypedValue for Vec<T> {
    fn encode(&self) -> Value {
        let elements = self.iter().map(|x| x.encode()).collect();

        Value::Array(ArrayValue { elements })
    }
}

#[derive(Default)]
pub struct TypedValueEncoder {
    fields: IndexMap<String, Value, RandomState>,
}

impl TypedValueEncoder {
    pub fn new() -> Self {
        Self { fields: IndexMap::new() }
    }

    pub fn add_field<T: EncodeAsTypedValue>(mut self, field: &str, value: &T) -> Self {
        self.fields.insert(field.to_string(), value.encode());
        self
    }

    pub fn build(self) -> Value {
        Value::Object(ObjectValue { fields: self.fields })
    }
}
