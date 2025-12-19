use crate::core::context::configuration::{Configuration, Profile};
use crate::core::context::environment::VariablesResolver;
use crate::core::Error;

pub mod configuration;
pub mod environment;

#[derive(Clone)]
pub struct Context {
    pub configuration: Configuration,
}

impl Context {
    pub fn new(configuration: Configuration) -> Context {
        Context { configuration }
    }

    pub fn load() -> Result<Self, Error> {
        let mut complete_profile = Profile::empty();

        let resolver = VariablesResolver::initialize();
        let environment = resolver.resolve_environment()?;
        let arguments = resolver.resolve_arguments()?;

        let profile_path = arguments
            .get("profile")
            .or_else(|| environment.get("profile"))
            .and_then(|x| x.as_str())
            .filter(|x| !x.is_empty());

        if profile_path.is_none() {
            println!(
                "No profile file specified.
Please provide a configuration profile using the `--profile` argument or the `PAYMASTER_PROFILE` environment variable, \
unless all variables are set via command line or environment variables."
            );
        }

        let profile = profile_path.map(Profile::from_file).unwrap_or(Ok(Profile::empty()))?;

        complete_profile.merge(&profile);
        complete_profile.insert_variables(environment)?;
        complete_profile.insert_variables(arguments)?;

        Configuration::from_profile(&complete_profile).map(Self::new)
    }
}

impl Into<paymaster_rpc::Configuration> for Context {
    fn into(self) -> paymaster_rpc::Configuration {
        paymaster_rpc::Configuration {
            rpc: self.configuration.rpc.clone(),

            forwarder: self.configuration.forwarder,
            gas_tank: self.configuration.gas_tank,

            supported_tokens: self.configuration.supported_tokens.clone(),

            max_fee_multiplier: self.configuration.max_fee_multiplier,
            provider_fee_overhead: self.configuration.provider_fee_overhead,

            estimate_account: self.configuration.estimate_account,

            relayers: self.configuration.relayers.clone(),

            starknet: self.configuration.starknet.clone(),
            price: self.configuration.clone().into(),
            sponsoring: self.configuration.sponsoring,
        }
    }
}
