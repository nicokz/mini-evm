use mini_evm::opcodes::*;
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;

fn delegate_call(ret_size: u8, target: u8) -> Vec<u8> {
    vec![
        PUSH1,
        ret_size,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        target,
        PUSH2,
        0x03,
        0xe8,
        DELEGATECALL,
    ]
}

fn delegate_call_with_gas(ret_size: u8, target: u8, gas: u16) -> Vec<u8> {
    vec![
        PUSH1,
        ret_size,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        target,
        PUSH2,
        (gas >> 8) as u8,
        gas as u8,
        DELEGATECALL,
    ]
}

#[test]
fn delegatecall_sstores_in_parent_storage_context() {
    let child_code = vec![PUSH1, 42, PUSH1, 1, SSTORE];
    let mut vm = Evm::new(&delegate_call_with_gas(0, 5, 30_000));
    vm.contracts.insert(
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5],
        child_code,
    );
    vm.storage_address[19] = 0xaa;

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ONE);
    assert_eq!(vm.storage.get(&U256::from(1u8)), Some(&U256::from(42u8)));
}

#[test]
fn delegatecall_preserves_address_caller_and_value() {
    let mut target = [0u8; 20];
    target[19] = 5;
    let parent_address = [0xabu8; 20];
    let caller = [0xcdu8; 20];
    let mut vm = Evm::new(&delegate_call(32, 5));
    vm.contracts.insert(
        target,
        vec![ADDRESS, PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN],
    );
    vm.context.address = parent_address;
    vm.storage_address = parent_address;
    vm.context.caller = caller;
    vm.context.value = 0x1234;

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ONE);
    let mut expected_address = [0u8; 32];
    expected_address[12..].copy_from_slice(&parent_address);
    assert_eq!(
        U256::from_be_bytes::<32>(vm.memory[0..32].try_into().unwrap()),
        U256::from_be_bytes(expected_address)
    );

    for (opcode, expected) in [(CALLER, caller), (CALLVALUE, [0u8; 20])] {
        let mut context_vm = Evm::new(&delegate_call(32, 5));
        context_vm.contracts.insert(
            target,
            vec![opcode, PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN],
        );
        context_vm.context.address = parent_address;
        context_vm.storage_address = parent_address;
        context_vm.context.caller = caller;
        context_vm.context.value = 0x1234;
        assert_eq!(context_vm.run(), ExecutionResult::Halt);
        assert_eq!(context_vm.stack.pop().unwrap(), U256::ONE);
        let actual = U256::from_be_bytes::<32>(context_vm.memory[0..32].try_into().unwrap());
        if opcode == CALLVALUE {
            assert_eq!(actual, U256::from(0x1234u16));
        } else {
            let mut expected_bytes = [0u8; 32];
            expected_bytes[12..].copy_from_slice(&expected);
            assert_eq!(actual, U256::from_be_bytes(expected_bytes));
        }
    }
}
