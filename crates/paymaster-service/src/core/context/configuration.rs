use std::collections::HashSet;
use std::fs;
use std::str::FromStr;

use paymaster_common::service::monitoring::Configuration as MonitoringConfiguration;
use paymaster_prices::Configuration as PriceConfiguration;
use paymaster_relayer::RelayersConfiguration;
use paymaster_sponsoring::Configuration as SponsoringConfiguration;
use paymaster_starknet::{Configuration as StarknetConfiguration, StarknetAccountConfiguration};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use serde_with::serde_as;
use starknet::core::types::Felt;

use crate::core::context::environment::{JSONPath, Variables};
use crate::core::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbosityConfiguration {
    Debug,
    Info,
}

impl FromStr for VerbosityConfiguration {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "debug" => Ok(VerbosityConfiguration::Debug),
            "info" => Ok(VerbosityConfiguration::Info),
            _ => Ok(VerbosityConfiguration::Debug),
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub verbosity: VerbosityConfiguration,
    pub prometheus: Option<MonitoringConfiguration>,

    pub rpc: paymaster_rpc::RPCConfiguration,

    pub forwarder: Felt,
    pub supported_tokens: HashSet<Felt>,

    pub max_fee_multiplier: f32,
    pub provider_fee_overhead: f32,

    pub estimate_account: StarknetAccountConfiguration,
    pub gas_tank: StarknetAccountConfiguration,

    pub relayers: RelayersConfiguration,

    pub starknet: StarknetConfiguration,
    pub price: PriceConfiguration,
    pub sponsoring: SponsoringConfiguration,
}

impl Configuration {
    #[allow(dead_code)]
    pub fn from_file(path: &str) -> Result<Self, Error> {
        let data = fs::read(path).map_err(|e| Error::Configuration(e.to_string()))?;

        serde_json::from_slice(&data).map_err(|e| Error::Configuration(e.to_string()))
    }

    pub fn from_profile(profile: &Profile) -> Result<Self, Error> {
        let data = serde_json::to_string(&profile.0).map_err(|e| Error::Configuration(e.to_string()))?;

        serde_json::from_str(&data).map_err(|e| Error::Configuration(e.to_string()))
    }

    #[allow(dead_code)]
    pub fn write_to_file(&self, path: &str) -> Result<(), Error> {
        // Write configuration to file
        let data = serde_json::to_string_pretty(&self).map_err(|e| Error::Configuration(e.to_string()))?;

        fs::write(path, data).map_err(|e| Error::Configuration(e.to_string()))
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct Profile(Map<String, Value>);

impl Profile {
    pub fn empty() -> Self {
        Self(Map::new())
    }

    pub fn from_file(path: &str) -> Result<Self, Error> {
        let data = fs::read(path).map_err(|e| Error::Configuration(e.to_string()))?;
        let variables: Map<String, Value> = serde_json::from_slice(&data).map_err(|e| Error::Configuration(e.to_string()))?;

        Ok(Self(variables))
    }

    pub fn merge(&mut self, profile: &Profile) {
        #[rustfmt::skip]
        fn merge_rec(profile: &mut Map<String, Value>, other: &Map<String, Value>) {
            for (k, v) in other {
                match (profile.get_mut(k), v) {
                    (Some(Value::Object(a_obj)), Value::Object(b_obj)) => { merge_rec(a_obj, b_obj); },
                    _ => { profile.insert(k.clone(), v.clone()); },
                }
            }
        }

        merge_rec(&mut self.0, &profile.0)
    }

    pub fn insert_variables(&mut self, variables: Variables) -> Result<(), Error> {
        for (key, value) in variables.into_iter() {
            self.insert_variable(key, value)?
        }

        Ok(())
    }

    pub fn insert_variable(&mut self, path: JSONPath, value: Value) -> Result<(), Error> {
        fn insert_rec(object: &mut Map<String, Value>, path: &[String], value: Value) -> Result<(), Error> {
            if path.len() == 1 {
                object.insert(path[0].to_string(), value);
                return Ok(());
            }

            let inner = object
                .entry(path[0].to_string())
                .or_insert(Value::Object(Map::new()))
                .as_object_mut()
                .ok_or(Error::Configuration(format!("could not merge variable {} in configuration", path[0])))?;

            insert_rec(inner, &path[1..], value)
        }

        insert_rec(&mut self.0, &path, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_from_str() {
        assert!(matches!(VerbosityConfiguration::from_str("debug"), Ok(VerbosityConfiguration::Debug)));
        assert!(matches!(VerbosityConfiguration::from_str("info"), Ok(VerbosityConfiguration::Info)));
        assert!(matches!(VerbosityConfiguration::from_str("unknown"), Ok(VerbosityConfiguration::Debug)));
    }

    use serde_json::{Map, Value};

    use crate::core::context::configuration::Profile;
    use crate::core::context::environment::JSONPath;

    #[test]
    fn insert_is_working_properly() {
        let expected: Map<String, Value> = serde_json::from_str(
            r#"{
            "foo_1": "42",
            "foo_2": "42",
            "foo_3": {
                "foo_1": "42",
                "foo_2": "42",
                "foo_3": {
                    "foo_1": "42"
                }
            }
        }"#,
        )
        .unwrap();

        let mut profile = Profile::empty();

        let value = Value::String("42".to_string());
        profile.insert_variable(JSONPath::from_str("foo_1"), value.clone()).unwrap();
        profile.insert_variable(JSONPath::from_str("foo_2"), value.clone()).unwrap();
        profile
            .insert_variable(JSONPath::from_str("foo_3.foo_1"), value.clone())
            .unwrap();
        profile
            .insert_variable(JSONPath::from_str("foo_3.foo_2"), value.clone())
            .unwrap();
        profile
            .insert_variable(JSONPath::from_str("foo_3.foo_3.foo_1"), value.clone())
            .unwrap();

        assert_eq!(profile.0, expected);
    }
}
