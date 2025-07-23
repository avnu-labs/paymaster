use std::collections::hash_map::IntoIter;
use std::collections::HashMap;
use std::env;
use std::ops::Deref;
use std::str::FromStr;

use lazy_static::lazy_static;
use serde_json::{Number, Value};

use crate::core::Error;

static CONFIGURATION_SPECIFICATION: &str = include_str!("../../../../../resources/specification/configuration.json");

lazy_static! {
    static ref IS_ARGUMENT: regex::Regex = regex::Regex::new(r"^--[^=]+=.+$").expect("invalid regex");
}

lazy_static! {
    static ref IS_STRING: regex::Regex = regex::Regex::new(r"^'[^']*'$").expect("invalid regex");
    static ref IS_NUMBER: regex::Regex = regex::Regex::new(r"^[0-9]+(\.[0-9]+)?$").expect("invalid regex");
    static ref IS_EMPTY_ARRAY: regex::Regex = regex::Regex::new(r"^\[\]$").expect("invalid regex");
    static ref IS_ARRAY: regex::Regex = regex::Regex::new(r"^\[.*\]$").expect("invalid regex");
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct JSONPath(Vec<String>);

impl Deref for JSONPath {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl JSONPath {
    pub fn from_str(s: &str) -> Self {
        JSONPath(s.split(".").map(|x| x.to_lowercase().to_string()).collect())
    }
}

#[derive(Debug)]
pub struct VariablesResolver(HashMap<String, JSONPath>);

impl VariablesResolver {
    pub fn initialize() -> Self {
        fn resolve_variables(path: &[String], value: Value) -> HashMap<String, JSONPath> {
            let mut variables = HashMap::new();
            match value {
                Value::Object(fields) => {
                    for (field, value) in fields {
                        variables.extend(resolve_variables(&[path, &[field]].concat(), value))
                    }
                },
                _ => {
                    variables.insert(path.join("_"), JSONPath(path.to_vec()));
                },
            }

            variables
        }

        let specification: Value = serde_json::from_str(CONFIGURATION_SPECIFICATION).expect("invalid specification");

        let mut resolutions = HashMap::new();
        resolutions.insert("profile".to_string(), JSONPath::from_str("profile"));
        resolutions.extend(resolve_variables(&[], specification));

        Self(resolutions)
    }

    pub fn resolve_environment(&self) -> Result<Variables, Error> {
        let variables = envy::prefixed("PAYMASTER_")
            .from_env::<HashMap<String, String>>()
            .map_err(|e| Error::Configuration(e.to_string()))?;

        self.resolve_variables(variables)
    }

    pub fn resolve_arguments(&self) -> Result<Variables, Error> {
        let raw_arguments = env::args();

        let mut arguments = HashMap::new();
        for raw_argument in raw_arguments.skip(1) {
            if !IS_ARGUMENT.is_match(&raw_argument) {
                return Err(Error::Configuration(format!("invalid argument {}, must be of the form '--xxx=yyy'", raw_argument)));
            }

            let Some((raw_name, raw_value)) = raw_argument.split_once("=") else { continue };

            let name = raw_name.trim().replace("--", "");
            let value = raw_value.to_string();

            arguments.insert(name, value);
        }

        self.resolve_variables(arguments)
    }

    fn resolve_variables(&self, variables: HashMap<String, String>) -> Result<Variables, Error> {
        let mut resolved_variables = HashMap::new();
        for (name, value) in variables {
            if let Some(path) = self.0.get(&name) {
                resolved_variables.insert(path.clone(), Self::decode_value(&value)?);
            }
        }

        Ok(Variables(resolved_variables))
    }

    fn decode_value(value: &str) -> Result<Value, Error> {
        Ok(match value {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),

            value if IS_STRING.is_match(value) => Value::String(value[1..value.len() - 1].to_string()),
            value if IS_NUMBER.is_match(value) => Number::from_str(value)
                .map(Value::Number)
                .map_err(|e| Error::Configuration(e.to_string()))?,
            value if IS_EMPTY_ARRAY.is_match(value) => Value::Array(vec![]),
            value if IS_ARRAY.is_match(value) => {
                let mut elements = vec![];
                for value in value[1..value.len() - 1].split(",") {
                    elements.push(Self::decode_value(value)?)
                }

                Value::Array(elements)
            },

            value => Value::String(value.to_string()),
        })
    }
}

pub struct Variables(HashMap<JSONPath, Value>);

impl From<HashMap<JSONPath, Value>> for Variables {
    fn from(map: HashMap<JSONPath, Value>) -> Self {
        Variables(map)
    }
}

impl Variables {
    pub fn get(&self, s: &str) -> Option<&Value> {
        self.0.get(&JSONPath::from_str(s))
    }

    pub fn into_iter(self) -> IntoIter<JSONPath, Value> {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Number, Value};

    use super::{JSONPath, VariablesResolver};

    #[test]
    fn key_from_variable_works_properly() {
        assert_eq!(JSONPath::from_str("foo-1").0, vec!["foo-1".to_string()]);
        assert_eq!(JSONPath::from_str("Foo-1").0, vec!["foo-1".to_string()]);
        assert_eq!(JSONPath::from_str("FOO-1").0, vec!["foo-1".to_string()]);

        assert_eq!(JSONPath::from_str("foo-1.bar-2").0, vec!["foo-1".to_string(), "bar-2".to_string()]);
        assert_eq!(JSONPath::from_str("Foo-1.Bar-2").0, vec!["foo-1".to_string(), "bar-2".to_string()]);
        assert_eq!(JSONPath::from_str("FOO-1.BAR-2").0, vec!["foo-1".to_string(), "bar-2".to_string()]);
    }

    #[test]
    fn parse_is_working_properly() {
        let cases = vec![
            ("number_1".to_lowercase(), "0".to_string(), Value::Number(Number::from(0))),
            ("number_2".to_lowercase(), "94".to_string(), Value::Number(Number::from(94))),
            ("number_3".to_lowercase(), "94.65".to_string(), Value::Number(Number::from_f64(94.65).unwrap())),
            ("true".to_lowercase(), "true".to_string(), Value::Bool(true)),
            ("false".to_lowercase(), "false".to_string(), Value::Bool(false)),
            ("string_1".to_lowercase(), "''".to_string(), Value::String("".to_string())),
            ("string_2".to_lowercase(), "'foo'".to_string(), Value::String("foo".to_string())),
            ("array_1".to_lowercase(), "[]".to_string(), Value::Array(vec![])),
            (
                "array_2".to_lowercase(),
                "[94,95,96]".to_string(),
                Value::Array(vec![
                    Value::Number(Number::from(94)),
                    Value::Number(Number::from(95)),
                    Value::Number(Number::from(96)),
                ]),
            ),
            (
                "array_3".to_lowercase(),
                "['foo','bar']".to_string(),
                Value::Array(vec![Value::String("foo".to_string()), Value::String("bar".to_string())]),
            ),
            ("any".to_lowercase(), "foo_bar".to_string(), Value::String("foo_bar".to_string())),
        ];

        let mut resolver = VariablesResolver(HashMap::new());
        for (case, _, _) in &cases {
            resolver.0.insert(case.to_string(), JSONPath::from_str(case));
        }

        for (case, value, expected) in cases {
            let result = resolver.resolve_variables(HashMap::from([(case.to_string(), value)])).unwrap();
            assert_eq!(result.get(&case).unwrap().clone(), expected)
        }
    }
}
