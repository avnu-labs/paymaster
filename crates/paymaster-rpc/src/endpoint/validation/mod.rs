use std::collections::HashSet;

use starknet::core::types::Felt;

use crate::endpoint::build::TransactionParameters;
use crate::endpoint::common::ExecutionParameters;
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

pub async fn check_is_allowed_fee_mode(ctx: &RequestContext<'_>, params: &ExecutionParameters) -> Result<(), Error> {
    if !params.fee_mode().is_sponsored() {
        return Ok(());
    }

    ctx.validate_api_key().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use jsonrpsee::Extensions;
    use paymaster_sponsoring::{Client as AuthenticationClient, Configuration, SelfConfiguration};
    use paymaster_starknet::constants::Token;

    use crate::endpoint::common::{ExecutionParameters, FeeMode, TipPriority};
    use crate::endpoint::validation::check_is_allowed_fee_mode;
    use crate::endpoint::RequestContext;
    use crate::middleware::APIKey;
    use crate::testing::TestEnvironment;

    fn params(fee_mode: FeeMode) -> ExecutionParameters {
        ExecutionParameters::V1 { fee_mode, time_bounds: None }
    }

    // TODO: enable when we can fix starknet image
    #[ignore]
    #[tokio::test]
    #[rustfmt::skip]
    async fn self_sponsoring_is_working_properly() {
        let test = TestEnvironment::new().await;
        let mut context = test.context().clone();
        let config = SelfConfiguration {api_key: "paymaster_123456".to_string(), sponsor_metadata: vec![],};
        context.sponsoring = AuthenticationClient::new(&Configuration::SelfSponsoring(config));
    
        let no_api_key = RequestContext::new(&context, &Extensions::default());
        let dummy_api_key = {
            let mut extensions = Extensions::new();
            extensions.insert(APIKey::new("paymaster_123456"));
            
            RequestContext::new(&context, &extensions)
        };
        
        let eth = Token::ETH_ADDRESS;
        check_is_allowed_fee_mode(&no_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        check_is_allowed_fee_mode(&dummy_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        assert!(check_is_allowed_fee_mode(&no_api_key, &params(FeeMode::Sponsored{ tip: TipPriority::Normal})).await.is_err());
        check_is_allowed_fee_mode(&dummy_api_key, &params(FeeMode::Sponsored{ tip: TipPriority::Normal})).await.unwrap();
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
        check_is_allowed_fee_mode(&no_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        check_is_allowed_fee_mode(&granted_api_key, &params(FeeMode::Default { gas_token: eth, tip: TipPriority::Normal })).await.unwrap();
        
        assert!(check_is_allowed_fee_mode(&no_api_key, &params(FeeMode::Sponsored { tip: TipPriority::Normal})).await.is_err());
        assert!(check_is_allowed_fee_mode(&dummy_api_key, &params(FeeMode::Sponsored{ tip: TipPriority::Normal})).await.is_err());
    }
}
