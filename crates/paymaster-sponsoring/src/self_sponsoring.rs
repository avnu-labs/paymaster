use starknet::core::types::Felt;

use crate::Error::InvalidApiKey;
use crate::{AuthenticatedApiKey, Error, SelfConfiguration};

#[derive(Clone)]
pub struct SelfSponsoring {
    api_key: String,
    sponsor_metadata: Vec<Felt>,
    allow_pool_fee_sponsoring: bool,
}

impl SelfSponsoring {
    pub fn new(configuration: SelfConfiguration) -> Result<Self, Error> {
        if !configuration.api_key.starts_with("paymaster_") {
            Err(InvalidApiKey("API key must start with 'paymaster_'".to_string()))?
        }
        Ok(Self {
            api_key: configuration.api_key,
            sponsor_metadata: configuration.sponsor_metadata,
            allow_pool_fee_sponsoring: configuration.allow_pool_fee_sponsoring,
        })
    }

    pub fn validate(&self, key: &str) -> AuthenticatedApiKey {
        if key == self.api_key {
            AuthenticatedApiKey {
                is_valid: true,
                sponsor_metadata: self.sponsor_metadata.clone(),
                allow_pool_fee_sponsoring: self.allow_pool_fee_sponsoring,
            }
        } else {
            AuthenticatedApiKey::invalid()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod new {
        use std::vec;

        use super::*;

        #[test]
        fn should_init_internal_authentication() {
            // Given
            let key = "paymaster_123456";
            let config = SelfConfiguration {
                api_key: key.to_string(),
                sponsor_metadata: vec![Felt::ZERO],
                allow_pool_fee_sponsoring: false,
            };

            // When
            let auth = SelfSponsoring::new(config).unwrap();

            // Then
            assert_eq!(&auth.api_key, &key);
            assert_eq!(&auth.sponsor_metadata, &vec![Felt::ZERO]);
        }
    }

    #[cfg(test)]
    mod validate {
        use std::vec;

        use super::*;

        #[test]
        fn should_return_valid_status_when_key_matches() {
            // Given
            let key = "paymaster_123456";
            let config = SelfConfiguration {
                api_key: key.to_string(),
                sponsor_metadata: vec![],
                allow_pool_fee_sponsoring: false,
            };
            let auth = SelfSponsoring::new(config).unwrap();

            // When
            let status = auth.validate("paymaster_123456");

            // Then
            assert!(status.is_valid);
            assert_eq!(&status.sponsor_metadata, &auth.sponsor_metadata);
            assert!(!status.allow_pool_fee_sponsoring);
        }

        #[test]
        fn should_propagate_pool_fee_sponsoring_permission() {
            // Given
            let key = "paymaster_123456";
            let config = SelfConfiguration {
                api_key: key.to_string(),
                sponsor_metadata: vec![],
                allow_pool_fee_sponsoring: true,
            };
            let auth = SelfSponsoring::new(config).unwrap();

            // When
            let status = auth.validate("paymaster_123456");

            // Then
            assert!(status.is_valid);
            assert!(status.allow_pool_fee_sponsoring);
        }

        #[test]
        fn should_default_pool_fee_sponsoring_permission_to_false() {
            let config: SelfConfiguration = serde_json::from_str(r#"{"api_key": "paymaster_1", "sponsor_metadata": []}"#).unwrap();

            assert!(!config.allow_pool_fee_sponsoring);
        }

        #[test]
        fn should_return_invalid_status_when_key_does_not_match() {
            // Given
            let key = "paymaster_123456";
            let config = SelfConfiguration {
                api_key: key.to_string(),
                sponsor_metadata: vec![],
                allow_pool_fee_sponsoring: false,
            };
            let auth = SelfSponsoring::new(config).unwrap();

            // When
            let status = auth.validate("paymaster_wrong_key");

            // Then
            assert!(!status.is_valid);
            assert_eq!(status.sponsor_metadata, vec![]);
        }
    }
}
