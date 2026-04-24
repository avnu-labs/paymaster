use starknet::ContractAddress;

#[starknet::interface]
pub trait IMockPrivacyPool<TStorage> {
    fn get_fee_amount(self: @TStorage) -> u128;
    fn apply_actions(ref self: TStorage);
}


#[starknet::contract]
pub mod MockPrivacyPool {
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use super::IMockPrivacyPool;

    #[storage]
    struct Storage {
        fee_amount: u128,
    }

    #[constructor]
    fn constructor(ref self: ContractState, fee_amount: u128) {
        self.fee_amount.write(fee_amount);
    }

    #[abi(embed_v0)]
    impl PoolImpl of IMockPrivacyPool<ContractState> {
        fn get_fee_amount(self: @ContractState) -> u128 {
            self.fee_amount.read()
        }

        fn apply_actions(ref self: ContractState) {}
    }
}
