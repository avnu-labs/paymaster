use starknet::core::types::typed_data::{ArrayValue, ObjectValue, Value};
use starknet::core::types::Felt;

use crate::Error;

pub trait DecodeFromTypedValue: Sized {
    fn decode(value: &Value) -> Result<Self, Error>;
}

impl DecodeFromTypedValue for Felt {
    fn decode(value: &Value) -> Result<Self, Error> {
        match value {
            Value::String(value) => Felt::from_hex(value).map_err(|e| Error::TypedDataDecoding(e.to_string())),
            _ => Err(Error::TypedDataDecoding("cannot decode value as felt".to_string())),
        }
    }
}

impl<T: DecodeFromTypedValue> DecodeFromTypedValue for Vec<T> {
    fn decode(value: &Value) -> Result<Self, Error> {
        let decoder = TypedValueDecoder::new(value);

        let mut values = vec![];
        for value in decoder.decode_array()? {
            values.push(T::decode(value.0)?)
        }

        Ok(values)
    }
}

pub struct TypedValueDecoder<'a>(&'a Value);

impl<'a> TypedValueDecoder<'a> {
    pub fn new(value: &'a Value) -> Self {
        Self(value)
    }

    pub fn decode<D: DecodeFromTypedValue>(&self) -> Result<D, Error> {
        D::decode(self.0)
    }

    pub fn decode_object(&self) -> Result<ObjectValueDecoder, Error> {
        match self.0 {
            Value::Object(v) => Ok(ObjectValueDecoder(v)),
            _ => Err(Error::TypedDataDecoding("value is not an object".to_string())),
        }
    }

    pub fn decode_array(&self) -> Result<ArrayValueDecoder, Error> {
        match self.0 {
            Value::Array(value) => Ok(ArrayValueDecoder { element: 0, value }),
            _ => Err(Error::TypedDataDecoding("value is not an object".to_string())),
        }
    }
}

pub struct ObjectValueDecoder<'a>(&'a ObjectValue);

impl<'a> ObjectValueDecoder<'a> {
    pub fn decode_field(&self, field: &str) -> Result<TypedValueDecoder<'a>, Error> {
        self.0
            .fields
            .get(field)
            .map(TypedValueDecoder)
            .ok_or(Error::TypedDataDecoding(format!("field {} does not exist", field)))
    }
}

#[derive(Debug)]
pub struct ArrayValueDecoder<'a> {
    element: usize,
    value: &'a ArrayValue,
}

impl<'a> Iterator for ArrayValueDecoder<'a> {
    type Item = TypedValueDecoder<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.element >= self.value.elements.len() {
            return None;
        }

        let current = self.element;
        self.element += 1;

        self.value.elements.get(current).map(TypedValueDecoder::new)
    }
}
