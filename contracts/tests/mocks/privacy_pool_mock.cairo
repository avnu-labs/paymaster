use starknet::ContractAddress;

#[starknet::interface]
pub trait IMockPrivacyPool<TStorage> {
    fn get_fee_amount(self: @TStorage) -> u128;
    fn get_fee_collector(self: @TStorage) -> ContractAddress;
    fn apply_actions(ref self: TStorage);
}


#[starknet::contract]
pub mod MockPrivacyPool {
    use avnu_lib::interfaces::erc20::{IERC20Dispatcher, IERC20DispatcherTrait};
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use starknet::{ContractAddress, get_caller_address};
    use super::IMockPrivacyPool;

    #[storage]
    struct Storage {
        fee_amount: u128,
        fee_collector: ContractAddress,
        strk_token: ContractAddress,
    }

    #[constructor]
    fn constructor(
        ref self: ContractState, fee_amount: u128, fee_collector: ContractAddress, strk_token: ContractAddress,
    ) {
        self.fee_amount.write(fee_amount);
        self.fee_collector.write(fee_collector);
        self.strk_token.write(strk_token);
    }

    #[abi(embed_v0)]
    impl PoolImpl of IMockPrivacyPool<ContractState> {
        fn get_fee_amount(self: @ContractState) -> u128 {
            self.fee_amount.read()
        }

        fn get_fee_collector(self: @ContractState) -> ContractAddress {
            self.fee_collector.read()
        }

        // Mirrors privacy.cairo:collect_fee — pulls `fee_amount` STRK from the caller
        // (the forwarder in practice) to `fee_collector` using transferFrom.
        fn apply_actions(ref self: ContractState) {
            let fee: u128 = self.fee_amount.read();
            if fee > 0 {
                let token = IERC20Dispatcher { contract_address: self.strk_token.read() };
                let collector = self.fee_collector.read();
                token.transferFrom(get_caller_address(), collector, fee.into());
            }
        }
    }
}
