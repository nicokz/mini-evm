use mini_evm::opcodes::*;
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
