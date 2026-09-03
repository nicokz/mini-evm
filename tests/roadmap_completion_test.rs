use mini_evm::opcodes::*;
use mini_evm::state::AccountState;
use mini_evm::vm::{Evm, ExecutionResult, VmError};
use ruint::aliases::U256;

#[test]
fn environment_opcodes_read_configured_values() {
    let mut vm = Evm::new(&[GASPRICE, CHAINID, NUMBER, GASLIMIT]);
    vm.env.gas_price = U256::from(11u8);
    vm.env.chain_id = U256::from(12u8);
    vm.env.number = U256::from(13u8);
    vm.env.gas_limit = U256::from(14u8);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), 14);
    assert_eq!(vm.stack.pop().unwrap(), 13);
    assert_eq!(vm.stack.pop().unwrap(), 12);
    assert_eq!(vm.stack.pop().unwrap(), 11);
}

#[test]
fn cancun_smoke_covers_memory_accesses_transient_storage_and_selfdestruct() {
    let mut memory_vm = Evm::new(&[
        PUSH1, 0xaa, PUSH1, 0, MSTORE8, PUSH1, 3, PUSH1, 0, PUSH1, 1, MCOPY, STOP,
    ]);
    assert_eq!(memory_vm.run(), ExecutionResult::Halt);
    assert_eq!(&memory_vm.memory[..4], &[0xaa, 0xaa, 0, 0]);

    let mut blob_vm = Evm::new(&[PUSH1, 0, BLOBHASH, PUSH1, 1, BLOBHASH, BLOBBASEFEE, STOP]);
    blob_vm.context.blob_versioned_hashes = vec![[0x11; 32]];
    blob_vm.env.blob_base_fee = U256::from(17u8);
    assert_eq!(blob_vm.run(), ExecutionResult::Halt);
    assert_eq!(blob_vm.stack.pop(), Ok(U256::from(17u8)));
    assert_eq!(blob_vm.stack.pop(), Ok(U256::ZERO));
    assert_eq!(blob_vm.stack.pop(), Ok(U256::from_be_bytes([0x11; 32])));

    let mut transient_vm = Evm::new(&[PUSH1, 7, PUSH1, 1, TSTORE, PUSH1, 1, TLOAD, STOP]);
    assert_eq!(transient_vm.run(), ExecutionResult::Halt);
    assert_eq!(transient_vm.stack.pop(), Ok(U256::from(7u8)));
    assert!(transient_vm.state.transient_storage.is_empty());

    let address = [0x22; 20];
    let mut access_vm = Evm::new_with_gas(&[PUSH1, 1, SLOAD, PUSH1, 1, SLOAD], 5_000);
    access_vm.context.address = address;
    access_vm
        .state
        .set_storage(address, U256::from(1u8), U256::from(9u8));
    assert_eq!(access_vm.run(), ExecutionResult::Halt);
    assert_eq!(access_vm.gas_left, 2_794);

    let mut beneficiary = [0u8; 20];
    beneficiary[19] = 0x33;
    let mut lifecycle_vm = Evm::new(&[PUSH1, 0x33, SELFDESTRUCT]);
    lifecycle_vm.context.address = address;
    lifecycle_vm.state.set_balance(address, U256::from(10u8));
    lifecycle_vm.state.set_code(address, vec![STOP]);
    lifecycle_vm
        .state
        .set_storage(address, U256::from(1u8), U256::from(2u8));
    lifecycle_vm.created_in_transaction.insert(address);
    assert_eq!(lifecycle_vm.run(), ExecutionResult::Halt);
    assert_eq!(
        lifecycle_vm.state.get_account(&address),
        AccountState::default()
    );
    assert_eq!(
        lifecycle_vm.state.get_balance(&beneficiary),
        U256::from(10u8)
    );
}

#[test]
fn mcopy_zero_length_allows_unallocated_offsets() {
    let mut vm = Evm::new(&[PUSH1, 0, PUSH1, 0xff, PUSH1, 0xff, MCOPY, STOP]);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert!(vm.memory.is_empty());
}

#[test]
fn returndata_copy_rejects_reads_past_buffer() {
    let mut vm = Evm::new(&[PUSH1, 2, PUSH1, 2, PUSH1, 0, RETURNDATACOPY]);
    vm.return_data = vec![0xaa, 0xbb, 0xcc];

    assert_eq!(
        vm.run(),
        ExecutionResult::VmError(VmError::OutOfBoundsReturnData)
    );
}

#[test]
fn staticcall_child_sstore_fails_and_returns_zero() {
    let child_code = vec![PUSH1, 42, PUSH1, 1, SSTORE];
    let mut target = [0u8; 20];
    target[19] = 5;

    let parent_code = vec![
        PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 5, PUSH2, 0x03, 0xe8, STATICCALL,
    ];
    let mut vm = Evm::new(&parent_code);
    vm.contracts.insert(target, child_code);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ZERO);
    assert_eq!(vm.storage.get(&U256::from(1u8)), None);
}
