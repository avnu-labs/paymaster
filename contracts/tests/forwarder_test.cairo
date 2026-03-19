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

mod ExecuteCalls {
    use avnu_lib::interfaces::erc20::IERC20DispatcherTrait;
    use starknet::account::Call;
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_token, set_contract_address,
    };

    // selector!("mint")
    const MINT_SELECTOR: felt252 = 0x02f0b3c5710379609eb5495f1ecd348cb28167711b73609fe565a72734550354;

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_and_collect_gas_fees() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let gas_fees_recipient = forwarder.get_gas_fees_recipient();
        // Deploy token with no initial balance
        let gas_token = deploy_mock_token(contract_address_const::<0x0>(), 0);
        let gas_token_address = gas_token.contract_address;
        let gas_amount: u256 = 5_u256;

        // Calls: mint gas_amount tokens to the forwarder during execution
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 5, 0].span(),
            },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_calls(calls, gas_token_address, gas_amount);

        // Then
        assert(result == true, 'invalid result');
        let recipient_balance = gas_token.balanceOf(gas_fees_recipient);
        assert(recipient_balance == gas_amount, 'invalid recipient balance');
        let forwarder_balance = gas_token.balanceOf(forwarder.contract_address);
        assert(forwarder_balance == 0_u256, 'forwarder should be empty');
    }

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_multiple_calls_and_collect_gas_fees() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let gas_fees_recipient = forwarder.get_gas_fees_recipient();
        let gas_token = deploy_mock_token(contract_address_const::<0x0>(), 0);
        let gas_token_address = gas_token.contract_address;
        let gas_amount: u256 = 3_u256;

        // Calls: two mints, total 5 tokens but only 3 required as gas
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 2, 0].span(),
            },
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 3, 0].span(),
            },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_calls(calls, gas_token_address, gas_amount);

        // Then
        assert(result == true, 'invalid result');
        let recipient_balance = gas_token.balanceOf(gas_fees_recipient);
        assert(recipient_balance == gas_amount, 'invalid recipient balance');
        // Forwarder keeps the excess (5 - 3 = 2)
        let forwarder_balance = gas_token.balanceOf(forwarder.contract_address);
        assert(forwarder_balance == 2_u256, 'invalid forwarder balance');
    }

    #[test]
    #[available_gas(2000000)]
    #[should_panic(expected: ('Caller is not whitelisted', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_caller_is_not_whitelisted() {
        // Given
        let (forwarder, _, _) = deploy_forwarder();
        let gas_token_address = contract_address_const::<0x1>();
        let calls: Array<Call> = array![
            Call { to: contract_address_const::<0x1>(), selector: 0x0, calldata: array![].span() },
        ];
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.execute_calls(calls, gas_token_address, 1_u256);
    }

    #[test]
    #[available_gas(2000000000)]
    #[should_panic(expected: ('Insufficient gas payment', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_insufficient_gas_payment() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let gas_token = deploy_mock_token(contract_address_const::<0x0>(), 0);
        let gas_token_address = gas_token.contract_address;

        // Mint only 2 tokens but require 5
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 2, 0].span(),
            },
        ];
        set_contract_address(caller);

        // When & Then
        forwarder.execute_calls(calls, gas_token_address, 5_u256);
    }
}

mod ExecuteSponsoredCalls {
    use starknet::account::Call;
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_account, set_contract_address,
    };

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_single_call() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let account = deploy_mock_account();
        // name() selector
        let entrypoint: felt252 = 0x361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
        let calls: Array<Call> = array![
            Call { to: account.contract_address, selector: entrypoint, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_sponsored_calls(calls, sponsor_metadata);

        // Then
        assert(result == true, 'invalid result');
    }

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_multiple_calls() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let account = deploy_mock_account();
        let entrypoint: felt252 = 0x361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
        let calls: Array<Call> = array![
            Call { to: account.contract_address, selector: entrypoint, calldata: array![].span() },
            Call { to: account.contract_address, selector: entrypoint, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_sponsored_calls(calls, sponsor_metadata);

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
        let calls: Array<Call> = array![
            Call { to: contract_address_const::<0x1>(), selector: 0x0, calldata: array![].span() },
        ];
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.execute_sponsored_calls(calls, sponsor_metadata);
    }
}
