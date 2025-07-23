use avnu::forwarder::IForwarderDispatcherTrait;
use avnu_lib::components::ownable::IOwnableDispatcherTrait;
use avnu_lib::components::whitelist::IWhitelistDispatcherTrait;
use starknet::contract_address_const;
use starknet::testing::set_contract_address;
use super::helper::{deploy_forwarder, deploy_mock_account, deploy_mock_token};

mod GetGasFessRecipient {
    use super::{IForwarderDispatcherTrait, contract_address_const, deploy_forwarder};

    #[test]
    #[available_gas(2000000)]
    fn should_return_gas_fess_recipient() {
        // Given
        let (forwarder, _, _) = deploy_forwarder();
        let expected = contract_address_const::<0x2>();

        // When
        let result = forwarder.get_gas_fees_recipient();

        // Then
        assert(result == expected, 'invalid recipient');
    }
}

mod SetGasFessRecipient {
    use super::{IForwarderDispatcherTrait, IOwnableDispatcherTrait, contract_address_const, deploy_forwarder, set_contract_address};

    #[test]
    #[available_gas(2000000)]
    fn should_set_gas_fess_recipient() {
        // Given
        let (forwarder, ownable, _) = deploy_forwarder();
        let recipient_address = contract_address_const::<0x3>();
        set_contract_address(ownable.get_owner());

        // When
        let result = forwarder.set_gas_fees_recipient(recipient_address);

        // Then
        assert(result == true, 'invalid result');
        let new_recipient = forwarder.get_gas_fees_recipient();
        assert(new_recipient == recipient_address, 'invalid recipient');
    }

    #[test]
    #[available_gas(2000000)]
    #[should_panic(expected: ('Caller is not the owner', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_caller_is_not_the_owner() {
        // Given
        let (forwarder, _, _) = deploy_forwarder();
        let recipient_address = contract_address_const::<0x3>();
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.set_gas_fees_recipient(recipient_address);
    }
}

mod Execute {
    use avnu_lib::interfaces::erc20::IERC20DispatcherTrait;
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_account, deploy_mock_token, set_contract_address,
    };

    #[test]
    #[available_gas(2000000000)]
    fn should_execute() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let account = deploy_mock_account();
        let account_address = account.contract_address;
        let entrypoint: felt252 = 0x361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
        let calldata: Array<felt252> = array![];
        let gas_token = deploy_mock_token(account_address, 10);
        let gas_token_address = gas_token.contract_address;
        let gas_amount: u256 = 1_u256;
        set_contract_address(account_address);
        gas_token.transfer(forwarder.contract_address, gas_amount);
        set_contract_address(caller);

        // When
        let result = forwarder.execute(account_address, entrypoint, calldata, gas_token_address, gas_amount);

        // Then
        assert(result == true, 'invalid result');
    }

    #[test]
    #[available_gas(2000000)]
    #[should_panic(expected: ('Caller is not whitelisted', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_caller_is_not_whitelisted() {
        // Given
        let (forwarder, _, _) = deploy_forwarder();
        let account_address = contract_address_const::<0x1>();
        let entrypoint: felt252 = 0x0;
        let calldata: Array<felt252> = array![0x1, 0x2];
        let gas_token_address = contract_address_const::<0x1>();
        let gas_amount: u256 = 1_u256;
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.execute(account_address, entrypoint, calldata, gas_token_address, gas_amount);
    }
}

mod ExecuteSponsored {
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_account, set_contract_address,
    };

    #[test]
    #[available_gas(2000000000)]
    fn should_execute() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let account = deploy_mock_account();
        let account_address = account.contract_address;
        let entrypoint: felt252 = 0x361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
        let calldata: Array<felt252> = array![];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_sponsored(account_address, entrypoint, calldata, sponsor_metadata);

        // Then
        assert(result == true, 'invalid result');
    }

    #[test]
    #[available_gas(2000000)]
    #[should_panic(expected: ('Caller is not whitelisted', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_caller_is_not_whitelisted() {
        // Given
        let (forwarder, _, _) = deploy_forwarder();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        let account_address = contract_address_const::<0x1>();
        let entrypoint: felt252 = 0x0;
        let calldata: Array<felt252> = array![0x1, 0x2];
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.execute_sponsored(account_address, entrypoint, calldata, sponsor_metadata);
    }
}
