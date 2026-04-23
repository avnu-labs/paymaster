use avnu::forwarder::IForwarderDispatcherTrait;
use avnu_lib::components::ownable::IOwnableDispatcherTrait;
use avnu_lib::components::whitelist::IWhitelistDispatcherTrait;
use starknet::contract_address_const;
use starknet::testing::set_contract_address;
use super::helper::{deploy_forwarder, deploy_mock_account, deploy_mock_pool, deploy_mock_token};

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

mod ExecutePrivate {
    use avnu_lib::interfaces::erc20::IERC20DispatcherTrait;
    use starknet::account::Call;
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_pool, deploy_mock_token, set_contract_address,
    };

    // selector!("mint")
    const MINT_SELECTOR: felt252 = 0x02f0b3c5710379609eb5495f1ecd348cb28167711b73609fe565a72734550354;
    // selector!("apply_actions")
    const APPLY_ACTIONS_SELECTOR: felt252 = selector!("apply_actions");

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

        // Deploy a mock privacy pool — the last call of every private batch targets the pool
        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        // Calls: mint gas_amount tokens to the forwarder during execution, then call the pool
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 5, 0].span(),
            },
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_private(calls, gas_token_address, gas_amount);

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

        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        // Calls: two mints (total 5 tokens, only 3 required as gas) followed by the pool call
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
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_private(calls, gas_token_address, gas_amount);

        // Then — all received tokens (5) are forwarded, not just gas_amount (3)
        assert(result == true, 'invalid result');
        let recipient_balance = gas_token.balanceOf(gas_fees_recipient);
        assert(recipient_balance == 5_u256, 'invalid recipient balance');
        let forwarder_balance = gas_token.balanceOf(forwarder.contract_address);
        assert(forwarder_balance == 0_u256, 'forwarder should be empty');
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
        forwarder.execute_private(calls, gas_token_address, 1_u256);
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

        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        // Mint only 2 tokens but require 5, then call the pool
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 2, 0].span(),
            },
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When & Then
        forwarder.execute_private(calls, gas_token_address, 5_u256);
    }
}

mod ExecutePrivateSponsored {
    use avnu_lib::interfaces::erc20::IERC20DispatcherTrait;
    use starknet::account::Call;
    use super::{
        IForwarderDispatcherTrait, IOwnableDispatcherTrait, IWhitelistDispatcherTrait, contract_address_const, deploy_forwarder,
        deploy_mock_account, deploy_mock_pool, deploy_mock_token, set_contract_address,
    };

    // selector!("mint")
    const MINT_SELECTOR: felt252 = 0x02f0b3c5710379609eb5495f1ecd348cb28167711b73609fe565a72734550354;
    const APPLY_ACTIONS_SELECTOR: felt252 = selector!("apply_actions");

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_without_pool_fee() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let account = deploy_mock_account();
        let entrypoint: felt252 = 0x361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60;
        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        let calls: Array<Call> = array![
            Call { to: account.contract_address, selector: entrypoint, calldata: array![].span() },
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        let gas_token_address = contract_address_const::<0x0>();
        set_contract_address(caller);

        // When
        let result = forwarder.execute_private_sponsored(calls, gas_token_address, 0_u256, sponsor_metadata);

        // Then
        assert(result == true, 'invalid result');
    }

    #[test]
    #[available_gas(2000000000)]
    fn should_execute_and_collect_pool_fee() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let gas_fees_recipient = forwarder.get_gas_fees_recipient();
        // Deploy token with pre-existing balance on forwarder (simulates STRK held for pool fees)
        let gas_token = deploy_mock_token(forwarder.contract_address, 10);
        let gas_token_address = gas_token.contract_address;
        let pool_fee: u256 = 3_u256;

        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        // Calls: mint pool_fee tokens to the forwarder (simulates TransferTo from apply_actions), then the pool call
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 3, 0].span(),
            },
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When
        let result = forwarder.execute_private_sponsored(calls, gas_token_address, pool_fee, sponsor_metadata);

        // Then
        assert(result == true, 'invalid result');
        let recipient_balance = gas_token.balanceOf(gas_fees_recipient);
        assert(recipient_balance == pool_fee, 'invalid recipient balance');
        // Forwarder keeps pre-existing balance (10) but pool fee (3) was transferred
        let forwarder_balance = gas_token.balanceOf(forwarder.contract_address);
        assert(forwarder_balance == 10_u256, 'invalid forwarder balance');
    }

    #[test]
    #[available_gas(2000000000)]
    #[should_panic(expected: ('Insufficient pool fee payment', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_insufficient_pool_fee() {
        // Given
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        // Forwarder has pre-existing balance of 10 — must not count toward pool fee
        let gas_token = deploy_mock_token(forwarder.contract_address, 10);
        let gas_token_address = gas_token.contract_address;

        let pool = deploy_mock_pool(0_u128, contract_address_const::<0x123>(), contract_address_const::<0x0>());
        // Mint only 2 tokens but require 5 — pre-existing balance must not help; then the pool call
        let calls: Array<Call> = array![
            Call {
                to: gas_token_address,
                selector: MINT_SELECTOR,
                calldata: array![forwarder.contract_address.into(), 2, 0].span(),
            },
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When & Then
        forwarder.execute_private_sponsored(calls, gas_token_address, 5_u256, sponsor_metadata);
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
        let gas_token_address = contract_address_const::<0x0>();
        set_contract_address(contract_address_const::<0x1234>());

        // When & Then
        forwarder.execute_private_sponsored(calls, gas_token_address, 0_u256, sponsor_metadata);
    }

    #[test]
    #[available_gas(2000000000)]
    fn should_approve_pool_and_let_pool_pull_fee() {
        // Given — pool with fee=3, forwarder pre-holds 10 STRK so the pool's transferFrom succeeds
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let strk = deploy_mock_token(forwarder.contract_address, 10);
        let fee_collector = contract_address_const::<0xFEE>();
        let pool_fee: u128 = 3_u128;
        let pool = deploy_mock_pool(pool_fee, fee_collector, strk.contract_address);
        let calls: Array<Call> = array![
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When — gas_amount=0 so the sponsor path skips its own balance accounting; only the
        // pool's transferFrom moves tokens, proving the forwarder granted an allowance to the pool.
        let result = forwarder.execute_private_sponsored(calls, strk.contract_address, 0_u256, sponsor_metadata);

        // Then
        assert(result == true, 'invalid result');
        assert(strk.balanceOf(fee_collector) == 3_u256, 'fee collector not paid');
        assert(strk.balanceOf(forwarder.contract_address) == 7_u256, 'forwarder not debited');
    }

    #[test]
    #[available_gas(2000000000)]
    #[should_panic(expected: ('Pool fee exceeds limit', 'ENTRYPOINT_FAILED'))]
    fn should_fail_when_pool_fee_exceeds_limit() {
        // Given — pool with a fee just over MAX_POOL_FEE (10_000 STRK in FRI)
        let (forwarder, ownable, whitelist) = deploy_forwarder();
        let caller = contract_address_const::<0x999>();
        let sponsor_metadata: Array<felt252> = array!['SPONSOR_ID'];
        set_contract_address(ownable.get_owner());
        whitelist.set_whitelisted_address(caller, true);
        let strk = deploy_mock_token(forwarder.contract_address, 0);
        let fee_collector = contract_address_const::<0xFEE>();
        // 10_000 STRK + 1 FRI
        let pool_fee: u128 = 10_000_000_000_000_000_000_001_u128;
        let pool = deploy_mock_pool(pool_fee, fee_collector, strk.contract_address);
        let calls: Array<Call> = array![
            Call { to: pool.contract_address, selector: APPLY_ACTIONS_SELECTOR, calldata: array![].span() },
        ];
        set_contract_address(caller);

        // When & Then
        forwarder.execute_private_sponsored(calls, strk.contract_address, 0_u256, sponsor_metadata);
    }
}
