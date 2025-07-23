use std::ops::Deref;

use starknet::core::types::typed_data::{Domain, ElementTypeReference, FullTypeReference, InlineTypeReference, Revision, Types, Value};
use starknet::core::types::{Call, Felt, TypedData};
use starknet::core::utils::cairo_short_string_to_felt;

use crate::types::TypeBuilder;
use crate::values::decoding::{DecodeFromTypedValue, TypedValueDecoder};
use crate::values::encoding::TypedValueEncoder;

mod call;
pub use call::*;

mod gas;
pub use gas::TransactionGasEstimate;
use paymaster_common::enum_dispatch;

mod time;
mod version;

pub use time::TimeBounds;
pub use version::{PaymasterVersion, SupportedVersion};

use crate::{ChainID, Error, Signature};

#[derive(Debug, Clone)]
pub struct ExecuteFromOutsideParameters {
    pub chain_id: ChainID,
    pub caller: Felt,
    pub nonce: Felt,
    pub time_bounds: TimeBounds,
    pub calls: Calls,
}

struct CallsV1(Vec<Call>);
impl DecodeFromTypedValue for CallsV1 {
    fn decode(value: &Value) -> Result<Self, Error> {
        let decoder = TypedValueDecoder::new(value);
        let mut calls = vec![];
        for value in decoder.decode_array()? {
            let call = value.decode_object()?;

            calls.push(Call {
                to: call.decode_field("to")?.decode()?,
                selector: call.decode_field("selector")?.decode()?,
                calldata: call.decode_field("calldata")?.decode()?,
            })
        }

        Ok(CallsV1(calls))
    }
}
impl From<CallsV1> for Calls {
    fn from(value: CallsV1) -> Self {
        Calls::new(value.0)
    }
}

struct CallsV2(Vec<Call>);
impl DecodeFromTypedValue for CallsV2 {
    fn decode(value: &Value) -> Result<Self, Error> {
        let decoder = TypedValueDecoder::new(value);
        let mut calls = vec![];
        for value in decoder.decode_array()? {
            let call = value.decode_object()?;

            calls.push(Call {
                to: call.decode_field("To")?.decode()?,
                selector: call.decode_field("Selector")?.decode()?,
                calldata: call.decode_field("Calldata")?.decode()?,
            })
        }

        Ok(CallsV2(calls))
    }
}
impl From<CallsV2> for Calls {
    fn from(value: CallsV2) -> Self {
        Calls::new(value.0)
    }
}

#[derive(Debug, Clone)]
pub enum ExecuteFromOutsideMessage {
    V1(ExecuteFromOutsideMessageV1),
    V2(ExecuteFromOutsideMessageV2),
}

impl ExecuteFromOutsideMessage {
    pub fn new(version: PaymasterVersion, params: ExecuteFromOutsideParameters) -> Self {
        match version {
            PaymasterVersion::V1 => Self::V1(ExecuteFromOutsideMessageV1::new(params)),
            PaymasterVersion::V2 => Self::V2(ExecuteFromOutsideMessageV2::new(params)),
        }
    }

    pub fn from_typed_data(value: &TypedData) -> Result<Self, Error> {
        Ok(match value.revision() {
            Revision::V0 => Self::V1(ExecuteFromOutsideMessageV1::from_typed_data(value)?),
            Revision::V1 => Self::V2(ExecuteFromOutsideMessageV2::from_typed_data(value)?),
        })
    }

    pub fn to_typed_data(self) -> Result<TypedData, Error> {
        enum_dispatch!(self {
            Self::V1(message) |
            Self::V2(message) => message.to_typed_data()
        })
    }

    pub fn calls(&self) -> &Calls {
        enum_dispatch!(self {
            Self::V1(message) |
            Self::V2(message) => message.calls()
        })
    }

    pub fn to_call(&self, user_address: Felt, signature: &Signature) -> Call {
        match self {
            Self::V1(message) => message.to_call(user_address, signature),
            Self::V2(message) => message.to_call(user_address, signature),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecuteFromOutsideMessageV1(ExecuteFromOutsideParameters);

impl Deref for ExecuteFromOutsideMessageV1 {
    type Target = ExecuteFromOutsideParameters;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ExecuteFromOutsideMessageV1 {
    pub fn new(params: ExecuteFromOutsideParameters) -> Self {
        Self(params)
    }

    pub fn from_typed_data(value: &TypedData) -> Result<Self, Error> {
        let decoder = TypedValueDecoder::new(&value.message());
        let object_decoder = decoder.decode_object()?;

        Ok(Self(ExecuteFromOutsideParameters {
            chain_id: ChainID::from_felt(value.encoder().domain().chain_id)?,
            caller: object_decoder.decode_field("caller")?.decode()?,
            nonce: object_decoder.decode_field("nonce")?.decode()?,
            calls: object_decoder.decode_field("calls")?.decode::<CallsV1>()?.into(),
            time_bounds: TimeBounds {
                execute_after: object_decoder
                    .decode_field("execute_after")?
                    .decode::<Felt>()?
                    .try_into()
                    .map_err(|_| Error::TypedDataDecoding("cannot decode time bounds".to_string()))?,

                execute_before: object_decoder
                    .decode_field("execute_before")?
                    .decode::<Felt>()?
                    .try_into()
                    .map_err(|_| Error::TypedDataDecoding("cannot decode time bounds".to_string()))?,
            },
        }))
    }

    pub fn calls(&self) -> &Calls {
        &self.calls
    }

    /*pub fn extract_gas_token_transfer(&self, forwarder: &Felt) -> Result<TokenTransfer, Error> {
        self.calls.extract_gas_token_transfer(forwarder).ok_or(Error::MissingGasFeeTransferCall)
    }*/

    pub fn to_typed_data(self) -> Result<TypedData, Error> {
        let value = TypedData::new(
            Self::types(),
            Self::domain(&self.chain_id),
            InlineTypeReference::Custom("OutsideExecution".to_string()),
            self.to_value(),
        )?;

        Ok(value)
    }

    pub fn to_value(self) -> Value {
        let builder = TypedValueEncoder::new()
            .add_field("caller", &self.caller)
            .add_field("nonce", &self.nonce)
            .add_field("execute_after", &Felt::from(self.time_bounds.execute_after))
            .add_field("execute_before", &Felt::from(self.time_bounds.execute_before))
            .add_field("calls_len", &Felt::from(self.calls.len()));

        let mut calls = vec![];
        for call in self.calls.iter() {
            calls.push(
                TypedValueEncoder::new()
                    .add_field("to", &call.to)
                    .add_field("selector", &call.selector)
                    .add_field("calldata_len", &Felt::from(call.calldata.len()))
                    .add_field("calldata", &call.calldata)
                    .build(),
            )
        }

        builder.add_field("calls", &calls).build()
    }

    pub fn to_call(&self, user_address: Felt, signature: &[Felt]) -> Call {
        Call {
            to: user_address,
            selector: PaymasterVersion::V1.method_selector(),
            calldata: CalldataBuilder::new()
                .encode(&self.caller)
                .encode(&self.nonce)
                .encode(&self.time_bounds)
                .encode(&self.calls)
                .encode(&signature)
                .build(),
        }
    }

    fn domain(chain_id: &ChainID) -> Domain {
        Domain {
            name: cairo_short_string_to_felt("Account.execute_from_outside").unwrap(),
            version: Felt::ONE,
            chain_id: chain_id.as_felt(),
            revision: Revision::V0,
        }
    }

    fn types() -> Types {
        let mut builder = TypeBuilder::new();
        builder
            .add_definition("OutsideExecution")
            .add_field("caller", FullTypeReference::Felt)
            .add_field("nonce", FullTypeReference::Felt)
            .add_field("execute_after", FullTypeReference::Felt)
            .add_field("execute_before", FullTypeReference::Felt)
            .add_field("calls_len", FullTypeReference::Felt)
            .add_field("calls", FullTypeReference::Array(ElementTypeReference::Custom(String::from("OutsideCall"))))
            .register();

        builder
            .add_definition("OutsideCall")
            .add_field("to", FullTypeReference::Felt)
            .add_field("selector", FullTypeReference::Felt)
            .add_field("calldata_len", FullTypeReference::Felt)
            .add_field("calldata", FullTypeReference::Array(ElementTypeReference::Felt))
            .register();

        builder.build(Revision::V0)
    }
}

#[derive(Debug, Clone)]
pub struct ExecuteFromOutsideMessageV2(ExecuteFromOutsideParameters);

impl Deref for ExecuteFromOutsideMessageV2 {
    type Target = ExecuteFromOutsideParameters;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ExecuteFromOutsideMessageV2 {
    pub fn new(params: ExecuteFromOutsideParameters) -> Self {
        Self(params)
    }

    pub fn from_typed_data(value: &TypedData) -> Result<Self, Error> {
        let decoder = TypedValueDecoder::new(&value.message());
        let object_decoder = decoder.decode_object()?;

        Ok(Self(ExecuteFromOutsideParameters {
            chain_id: ChainID::from_felt(value.encoder().domain().chain_id)?,
            caller: object_decoder.decode_field("Caller")?.decode()?,
            nonce: object_decoder.decode_field("Nonce")?.decode()?,
            calls: object_decoder.decode_field("Calls")?.decode::<CallsV2>()?.into(),
            time_bounds: TimeBounds {
                execute_after: object_decoder
                    .decode_field("Execute After")?
                    .decode::<Felt>()?
                    .try_into()
                    .map_err(|_| Error::TypedDataDecoding("cannot decode time bounds".to_string()))?,

                execute_before: object_decoder
                    .decode_field("Execute Before")?
                    .decode::<Felt>()?
                    .try_into()
                    .map_err(|_| Error::TypedDataDecoding("cannot decode time bounds".to_string()))?,
            },
        }))
    }

    pub fn calls(&self) -> &Calls {
        &self.calls
    }

    /*pub fn extract_gas_token_transfer(&self, forwarder: &Felt) -> Result<TokenTransfer, Error> {
        self.calls.extract_gas_token_transfer(forwarder).ok_or(Error::MissingGasFeeTransferCall)
    }*/

    pub fn to_typed_data(self) -> Result<TypedData, Error> {
        let typed_data = TypedData::new(
            Self::types(),
            Self::domain(&self.chain_id),
            InlineTypeReference::Custom("OutsideExecution".to_string()),
            self.to_value(),
        )?;

        Ok(typed_data)
    }

    pub fn to_value(self) -> Value {
        let builder = TypedValueEncoder::new()
            .add_field("Caller", &self.caller)
            .add_field("Nonce", &self.nonce)
            .add_field("Execute After", &Felt::from(self.time_bounds.execute_after))
            .add_field("Execute Before", &Felt::from(self.time_bounds.execute_before));

        let mut calls = vec![];
        for call in self.calls.iter() {
            calls.push(
                TypedValueEncoder::new()
                    .add_field("To", &call.to)
                    .add_field("Selector", &call.selector)
                    .add_field("Calldata", &call.calldata)
                    .build(),
            )
        }

        builder.add_field("Calls", &calls).build()
    }

    pub fn to_call(&self, user: Felt, signature: &[Felt]) -> Call {
        Call {
            to: user,
            selector: PaymasterVersion::V2.method_selector(),
            calldata: CalldataBuilder::new()
                .encode(&self.caller)
                .encode(&self.nonce)
                .encode(&self.time_bounds)
                .encode(&self.calls)
                .encode(&signature)
                .build(),
        }
    }

    fn domain(chain_id: &ChainID) -> Domain {
        Domain {
            name: cairo_short_string_to_felt("Account.execute_from_outside").unwrap(),
            version: Felt::from(2),
            chain_id: chain_id.as_felt(),
            revision: Revision::V1,
        }
    }

    fn types() -> Types {
        let mut builder = TypeBuilder::new();
        builder
            .add_definition("OutsideExecution")
            .add_field("Caller", FullTypeReference::ContractAddress)
            .add_field("Nonce", FullTypeReference::Felt)
            .add_field("Execute After", FullTypeReference::U128)
            .add_field("Execute Before", FullTypeReference::U128)
            .add_field("Calls", FullTypeReference::Array(ElementTypeReference::Custom(String::from("Call"))))
            .register();

        builder
            .add_definition("Call")
            .add_field("To", FullTypeReference::ContractAddress)
            .add_field("Selector", FullTypeReference::Selector)
            .add_field("Calldata", FullTypeReference::Array(ElementTypeReference::Felt))
            .register();

        builder.build(Revision::V1)
    }
}
