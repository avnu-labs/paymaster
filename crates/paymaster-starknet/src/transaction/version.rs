use std::collections::HashMap;

use paymaster_common::concurrency::ConcurrentExecutor;
use paymaster_common::task;
use starknet::core::types::{Felt, FunctionCall};
use starknet::macros::selector;

use crate::contract::ContractClass;
use crate::{Client, Error};

const PAYMASTER_V1_INTERFACE_ID: Felt = Felt::from_raw([492161624466288994, 7331630999786889399, 16029490553032031222, 10189501558710363126]);
const PAYMASTER_V2_INTERFACE_ID: Felt = Felt::from_raw([150957962276023817, 11215169228216991143, 16086434234789672676, 1434039593026997526]);

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum PaymasterVersion {
    V1,
    V2,
}

pub struct SupportedVersion(HashMap<PaymasterVersion, bool>);

impl SupportedVersion {
    #[cfg(test)]
    pub fn from_versions(versions: HashMap<PaymasterVersion, bool>) -> Self {
        Self(versions)
    }

    pub fn support_version(&self, version: PaymasterVersion) -> bool {
        self.0.get(&version).cloned().unwrap_or_default()
    }

    pub fn minimum_version(&self) -> Option<PaymasterVersion> {
        self.0.iter().filter_map(|(x, is_supported)| is_supported.then_some(*x)).min()
    }

    pub fn maximum_version(&self) -> Option<PaymasterVersion> {
        self.0.iter().filter_map(|(x, is_supported)| is_supported.then_some(*x)).max()
    }
}

impl PaymasterVersion {
    pub fn from_class(class: &ContractClass) -> Result<PaymasterVersion, Error> {
        // Precedence here is important, because in general when an account support newer version it also
        // support older version for retro-compatibility
        match () {
            _ if class.abi.contains_selector(selector!("execute_from_outside_v2")) => Ok(PaymasterVersion::V2),
            _ if class.abi.contains_selector(selector!("execute_from_outside")) => Ok(PaymasterVersion::V1),
            _ => Err(Error::InvalidVersion),
        }
    }

    pub fn method_selector(&self) -> Felt {
        match self {
            PaymasterVersion::V1 => selector!("execute_from_outside"),
            PaymasterVersion::V2 => selector!("execute_from_outside_v2"),
        }
    }

    #[rustfmt::skip]
    pub async fn fetch_supported_version(starknet: &Client, user: Felt) -> Result<SupportedVersion, Error> {
        let results = ConcurrentExecutor::new(starknet.clone(), 8)
            .register(task!(|client| { Self::check_compatibility_v1(&client, user).await.map(|x| (PaymasterVersion::V1, x)) }))
            .register(task!(|client| { Self::check_compatibility_v2(&client, user).await.map(|x| (PaymasterVersion::V2, x)) }))
            .execute()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let compatibilities = results
            .into_iter()
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .fold(HashMap::new(), |mut x, y| { x.insert(y.0, y.1); x });

        Ok(SupportedVersion(compatibilities))
    }

    pub async fn check_compatibility_v1(starknet: &Client, user: Felt) -> Result<bool, Error> {
        Self::check_compatibility(starknet, user, PAYMASTER_V1_INTERFACE_ID).await
    }

    pub async fn check_compatibility_v2(starknet: &Client, user: Felt) -> Result<bool, Error> {
        Self::check_compatibility(starknet, user, PAYMASTER_V2_INTERFACE_ID).await
    }

    async fn check_compatibility(starknet: &Client, user: Felt, interface_id: Felt) -> Result<bool, Error> {
        let response = starknet
            .call(&FunctionCall {
                contract_address: user,
                entry_point_selector: selector!("supports_interface"),
                calldata: vec![interface_id],
            })
            .await;

        match response {
            Ok(value) => Ok(value.first() == Some(&Felt::ONE)),
            Err(e) => Err(Error::Starknet(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::transaction::{PaymasterVersion, SupportedVersion};

    #[test]
    fn maximum_version_return_correct_max_version() {
        let no_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, false), (PaymasterVersion::V2, false)]));

        assert_eq!(no_supported.maximum_version(), None);

        let all_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, true), (PaymasterVersion::V2, true)]));

        assert_eq!(all_supported.maximum_version(), Some(PaymasterVersion::V2));

        let some_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, true), (PaymasterVersion::V2, false)]));

        assert_eq!(some_supported.maximum_version(), Some(PaymasterVersion::V1));
    }

    #[test]
    fn minimum_version_return_correct_min_version() {
        let no_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, false), (PaymasterVersion::V2, false)]));

        assert_eq!(no_supported.minimum_version(), None);

        let all_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, true), (PaymasterVersion::V2, true)]));

        assert_eq!(all_supported.minimum_version(), Some(PaymasterVersion::V1));

        let some_supported = SupportedVersion::from_versions(HashMap::from([(PaymasterVersion::V1, false), (PaymasterVersion::V2, true)]));

        assert_eq!(some_supported.minimum_version(), Some(PaymasterVersion::V2));
    }
}
