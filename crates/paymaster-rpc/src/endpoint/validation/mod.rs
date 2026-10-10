use std::collections::HashSet;

use paymaster_sponsoring::AuthenticatedApiKey;
use starknet::core::types::Felt;

use crate::endpoint::build::TransactionParameters;
use crate::endpoint::common::{ExecutionParameters, FeeMode};
use crate::endpoint::RequestContext;
use crate::Error;

pub async fn check_service_is_available(ctx: &RequestContext<'_>) -> Result<(), Error> {
    if ctx.context.execution.get_relayer_manager().count_enabled_relayers().await == 0 {
        return Err(Error::ServiceNotAvailable);
    }

    Ok(())
}

pub fn check_no_blacklisted_call(transaction: &TransactionParameters, contracts_blacklist: &HashSet<Felt>) -> Result<(), Error> {
    let has_blacklisted_calls = transaction.calls().iter().any(|x| contracts_blacklist.contains(&x.to));
    if !has_blacklisted_calls {
        return Ok(());
    }

    Err(Error::BlacklistedCalls)
}

pub fn check_is_supported_token(transaction: &ExecutionParameters, supported_tokens: &HashSet<Felt>) -> Result<(), Error> {
    if supported_tokens.contains(&transaction.gas_token()) {
        return Ok(());
    }

    Err(Error::TokenNotSupported)
}

/// Authenticate and authorize the fee mode: every sponsored fee mode requires a valid API key, and
/// having the sponsor pay the pool fee additionally requires the key to be allowed to. Returns the
/// authenticated key for sponsored requests, `None` for gasless ones.
pub async fn authorize_fee_mode(ctx: &RequestContext<'_>, params: &ExecutionParameters) -> Result<Option<AuthenticatedApiKey>, Error> {
    if !params.fee_mode().is_sponsored() {
        return Ok(None);
    }

    let authenticated_api_key = ctx.validate_api_key().await?;
    check_api_key_allows_fee_mode(&authenticated_api_key, params)?;
    Ok(Some(authenticated_api_key))
}

/// The sponsor bears the pool fee when it pays it, so its backend must have allowed it for the key
pub fn check_api_key_allows_fee_mode(api_key: &AuthenticatedApiKey, params: &ExecutionParameters) -> Result<(), Error> {
    if params.fee_mode().is_pool_fee_sponsored() && !api_key.allow_pool_fee_sponsoring {
        return Err(Error::PoolFeeSponsoringNotAllowed);
    }

    Ok(())
}

/// A private transaction has a pool fee, so a sponsored request must say who pays it. Any other
/// transaction type has none, so a request must not say anything about it.
pub fn check_fee_mode_matches_transaction(params: &ExecutionParameters, is_private_transaction: bool) -> Result<(), Error> {
    match (params.fee_mode(), is_private_transaction) {
        (FeeMode::SponsoredPrivate { .. }, false) => Err(Error::SponsoredPrivateRequiresPrivacy),
        (FeeMode::Sponsored { pool_fee: Some(_), .. }, false) => Err(Error::PoolFeeRequiresPrivacy),
        (FeeMode::Sponsored { pool_fee: None, .. }, true) => Err(Error::PoolFeeRequired),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use jsonrpsee::Extensions;
    use paymaster_sponsoring::{AuthenticatedApiKey, Client as AuthenticationClient, Configuration, SelfConfiguration};
    use paymaster_starknet::constants::Token;

    use crate::endpoint::common::{ExecutionParameters, FeeMode, PoolFee, TipPriority};
    use crate::endpoint::validation::{authorize_fee_mode, check_api_key_allows_fee_mode, check_fee_mode_matches_transaction};
    use crate::endpoint::RequestContext;
    use crate::middleware::APIKey;
    use crate::testing::TestEnvironment;
    use crate::Error;

    fn params(fee_mode: FeeMode) -> ExecutionParameters {
        ExecutionParameters::V1 { fee_mode, time_bounds: None }
    }

    fn sponsored() -> ExecutionParameters {
        params(FeeMode::Sponsored {
            pool_fee: None,
            tip: TipPriority::Normal,
        })
    }

    fn pool_fee_paid_by_user() -> ExecutionParameters {
        params(FeeMode::Sponsored {
            pool_fee: Some(PoolFee::User { token: Token::ETH_ADDRESS }),
            tip: TipPriority::Normal,
        })
    }

    fn pool_fee_paid_by_sponsor() -> ExecutionParameters {
        params(FeeMode::Sponsored {
            pool_fee: Some(PoolFee::Sponsor),
            tip: TipPriority::Normal,
        })
    }

    fn legacy_sponsored_private() -> ExecutionParameters {
        params(FeeMode::SponsoredPrivate {
            pool_fee_token: Token::ETH_ADDRESS,
            tip: TipPriority::Normal,
        })
    }

    fn gasless() -> ExecutionParameters {
        params(FeeMode::Default {
            gas_token: Token::ETH_ADDRESS,
            tip: TipPriority::Normal,
        })
    }

    #[test]
    fn pool_fee_sponsoring_requires_the_api_key_permission() {
        let key = AuthenticatedApiKey::valid(vec![]);
        let allowed_key = AuthenticatedApiKey {
            allow_pool_fee_sponsoring: true,
            ..AuthenticatedApiKey::valid(vec![])
        };

        assert!(matches!(
            check_api_key_allows_fee_mode(&key, &pool_fee_paid_by_sponsor()),
            Err(Error::PoolFeeSponsoringNotAllowed)
        ));
        check_api_key_allows_fee_mode(&allowed_key, &pool_fee_paid_by_sponsor()).unwrap();

        // Other sponsored modes never need the permission
        check_api_key_allows_fee_mode(&key, &sponsored()).unwrap();
        check_api_key_allows_fee_mode(&key, &pool_fee_paid_by_user()).unwrap();
        check_api_key_allows_fee_mode(&key, &legacy_sponsored_private()).unwrap();
    }

    #[test]
    fn private_transactions_require_a_pool_fee_in_sponsored_mode() {
        check_fee_mode_matches_transaction(&gasless(), true).unwrap();
        check_fee_mode_matches_transaction(&pool_fee_paid_by_user(), true).unwrap();
        check_fee_mode_matches_transaction(&pool_fee_paid_by_sponsor(), true).unwrap();
        check_fee_mode_matches_transaction(&legacy_sponsored_private(), true).unwrap();

        assert!(matches!(check_fee_mode_matches_transaction(&sponsored(), true), Err(Error::PoolFeeRequired)));
    }

    #[test]
    fn other_transactions_must_not_mention_the_pool_fee() {
        check_fee_mode_matches_transaction(&gasless(), false).unwrap();
        check_fee_mode_matches_transaction(&sponsored(), false).unwrap();

        assert!(matches!(
            check_fee_mode_matches_transaction(&pool_fee_paid_by_user(), false),
            Err(Error::PoolFeeRequiresPrivacy)
        ));
        assert!(matches!(
            check_fee_mode_matches_transaction(&pool_fee_paid_by_sponsor(), false),
            Err(Error::PoolFeeRequiresPrivacy)
        ));
        assert!(matches!(
            check_fee_mode_matches_transaction(&legacy_sponsored_private(), false),
            Err(Error::SponsoredPrivateRequiresPrivacy)
        ));
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    #[rustfmt::skip]
    async fn self_sponsoring_is_working_properly() {
        let test = TestEnvironment::new().await;
        let mut context = test.context().clone();
        let config = SelfConfiguration {api_key: "paymaster_123456".to_string(), sponsor_metadata: vec![], allow_pool_fee_sponsoring: false,};
        context.sponsoring = AuthenticationClient::new(&Configuration::SelfSponsoring(config));
    
        let no_api_key = RequestContext::new(&context, &Extensions::default());
        let dummy_api_key = {
            let mut extensions = Extensions::new();
            extensions.insert(APIKey::new("paymaster_123456"));
            
            RequestContext::new(&context, &extensions)
        };
        
        let eth = Token::ETH_ADDRESS;
        authorize_fee_mode(&no_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        authorize_fee_mode(&dummy_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        assert!(authorize_fee_mode(&no_api_key, &params(FeeMode::Sponsored{ pool_fee: None, tip: TipPriority::Normal})).await.is_err());
        authorize_fee_mode(&dummy_api_key, &params(FeeMode::Sponsored{ pool_fee: None, tip: TipPriority::Normal})).await.unwrap();
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    #[rustfmt::skip]
    async fn gasless_only_access_is_working_properly() {
        let test = TestEnvironment::new().await;
        let mut context = test.context().clone();
        context.sponsoring = AuthenticationClient::new(&Configuration::None);
    
        let no_api_key = RequestContext::new(&context, &Extensions::default());
        let dummy_api_key = {
            let mut extensions = Extensions::new();
            extensions.insert(APIKey::new("dummy"));
            
            RequestContext::new(&context, &extensions)
        };
    
        let granted_api_key = {
            let mut extensions = Extensions::new();
            extensions.insert(APIKey::new("granted"));
            
            RequestContext::new(&context, &extensions)
        };
        let eth = Token::ETH_ADDRESS;
        authorize_fee_mode(&no_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        authorize_fee_mode(&granted_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        
        assert!(authorize_fee_mode(&no_api_key, &params(FeeMode::Sponsored { pool_fee: None, tip: TipPriority::Normal})).await.is_err());
        assert!(authorize_fee_mode(&dummy_api_key, &params(FeeMode::Sponsored{ pool_fee: None, tip: TipPriority::Normal})).await.is_err());
    }
}
