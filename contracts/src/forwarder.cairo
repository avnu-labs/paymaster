use starknet::ContractAddress;

#[starknet::interface]
pub trait IForwarder<TContractState> {
    fn get_gas_fees_recipient(self: @TContractState) -> ContractAddress;
    fn set_gas_fees_recipient(ref self: TContractState, gas_fees_recipient: ContractAddress) -> bool;
    fn execute(
        ref self: TContractState,
        account_address: ContractAddress,
        entrypoint: felt252,
        calldata: Array<felt252>,
        gas_token_address: ContractAddress,
        gas_amount: u256,
    ) -> bool;
    fn execute_sponsored(
        ref self: TContractState,
        account_address: ContractAddress,
        entrypoint: felt252,
        calldata: Array<felt252>,
        sponsor_metadata: Array<felt252>,
    ) -> bool;
}

#[starknet::contract]
pub mod Forwarder {
    use avnu_lib::components::ownable::OwnableComponent;
    use avnu_lib::components::ownable::OwnableComponent::OwnableInternalImpl;
    use avnu_lib::components::upgradable::UpgradableComponent;
    use avnu_lib::components::whitelist::WhitelistComponent;
    use avnu_lib::interfaces::erc20::{IERC20Dispatcher, IERC20DispatcherTrait};
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use starknet::syscalls::call_contract_syscall;
    use starknet::{ContractAddress, SyscallResultTrait, get_caller_address, get_contract_address};
    use super::IForwarder;

    component!(path: OwnableComponent, storage: ownable, event: OwnableEvent);
    component!(path: UpgradableComponent, storage: upgradable, event: UpgradableEvent);
    component!(path: WhitelistComponent, storage: whitelist, event: WhitelistEvent);

    #[abi(embed_v0)]
    impl OwnableImpl = OwnableComponent::OwnableImpl<ContractState>;

    #[abi(embed_v0)]
    impl UpgradableImpl = UpgradableComponent::UpgradableImpl<ContractState>;

    #[abi(embed_v0)]
    impl WhitelistImpl = WhitelistComponent::WhitelistImpl<ContractState>;

    #[storage]
    struct Storage {
        gas_fees_recipient: ContractAddress,
        #[substorage(v0)]
        ownable: OwnableComponent::Storage,
        #[substorage(v0)]
        upgradable: UpgradableComponent::Storage,
        #[substorage(v0)]
        whitelist: WhitelistComponent::Storage,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        #[flat]
        OwnableEvent: OwnableComponent::Event,
        #[flat]
        UpgradableEvent: UpgradableComponent::Event,
        #[flat]
        WhitelistEvent: WhitelistComponent::Event,
        SponsoredTransaction: SponsoredTransaction,
    }

    #[derive(Drop, starknet::Event, PartialEq)]
    pub struct SponsoredTransaction {
        pub user_address: ContractAddress,
        pub sponsor_metadata: Array<felt252>,
    }

    #[constructor]
    fn constructor(ref self: ContractState, owner: ContractAddress, gas_fees_recipient: ContractAddress) {
        self.ownable.initialize(owner);
        self.gas_fees_recipient.write(gas_fees_recipient);
    }

    #[abi(embed_v0)]
    impl ForwarderImpl of IForwarder<ContractState> {
        fn get_gas_fees_recipient(self: @ContractState) -> ContractAddress {
            self.gas_fees_recipient.read()
        }

        fn set_gas_fees_recipient(ref self: ContractState, gas_fees_recipient: ContractAddress) -> bool {
            self.ownable.assert_only_owner();
            self.gas_fees_recipient.write(gas_fees_recipient);
            true
        }

        fn execute(
            ref self: ContractState,
            account_address: ContractAddress,
            entrypoint: felt252,
            calldata: Array<felt252>,
            gas_token_address: ContractAddress,
            gas_amount: u256,
        ) -> bool {
            // Check if caller is whitelisted
            let caller = get_caller_address();
            assert(self.whitelist.is_whitelisted(caller), 'Caller is not whitelisted');

            // Execute the call
            call_contract_syscall(account_address, entrypoint, calldata.span()).unwrap_syscall();

            // Collect gas fees
            let contract_address = get_contract_address();
            let gas_token = IERC20Dispatcher { contract_address: gas_token_address };
            let gas_fees_recipient = self.get_gas_fees_recipient();
            gas_token.transfer(gas_fees_recipient, gas_amount);
            let gas_token_balance = gas_token.balanceOf(contract_address);
            gas_token.transfer(account_address, gas_token_balance);

            true
        }

        fn execute_sponsored(
            ref self: ContractState,
            account_address: ContractAddress,
            entrypoint: felt252,
            calldata: Array<felt252>,
            sponsor_metadata: Array<felt252>,
        ) -> bool {
            // Check if caller is whitelisted
            let caller = get_caller_address();
            assert(self.whitelist.is_whitelisted(caller), 'Caller is not whitelisted');

            // Execute the call
            call_contract_syscall(account_address, entrypoint, calldata.span()).unwrap_syscall();

            // Emit event
            self.emit(SponsoredTransaction { user_address: account_address, sponsor_metadata });
            true
        }
    }
}
