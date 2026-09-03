use mini_evm::opcodes::*;
use mini_evm::state::{AccountState, StateFork};
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;
use std::collections::HashMap;

fn address(last_byte: u8) -> [u8; 20] {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

fn delegatecall(target: u8) -> Vec<u8> {
    vec![
        PUSH1,
        32,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        target,
        PUSH2,
        0x27,
        0x10,
        DELEGATECALL,
    ]
}

#[test]
fn test_tstore_tload_basic() {
    let mut vm = Evm::new(&[PUSH1, 42, PUSH1, 1, TSTORE, PUSH1, 1, TLOAD, STOP]);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::from(42u8));
}

#[test]
fn test_transient_storage_persists_across_calls() {
    let target = address(10);
    let child = vec![
        PUSH1, 1, TLOAD, PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN,
    ];
    let mut vm = Evm::new(&[
        PUSH1,
        42,
        PUSH1,
        1,
        TSTORE,
        PUSH1,
        32,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        0,
        PUSH1,
        10,
        PUSH2,
        0x27,
        0x10,
        DELEGATECALL,
    ]);
    vm.state.base_state.insert(
        target,
        AccountState {
            code: child,
            ..AccountState::default()
        },
    );
    vm.context.address = address(9);
    vm.storage_address = address(9);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.memory[31], 42);
}

#[test]
fn test_transient_storage_reverts_on_subcall_revert() {
    let target = address(10);
    let child = vec![PUSH1, 99, PUSH1, 1, TSTORE, PUSH1, 0, PUSH1, 0, REVERT];
    let mut code = vec![PUSH1, 42, PUSH1, 1, TSTORE];
    code.extend(delegatecall(10));
    code.extend([PUSH1, 1, TLOAD, STOP]);
    let mut vm = Evm::new(&code);
    vm.state.base_state.insert(
        target,
        AccountState {
            code: child,
            ..AccountState::default()
        },
    );
    vm.context.address = address(9);
    vm.storage_address = address(9);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::from(42u8));
}

#[test]
fn test_staticcall_rejects_tstore() {
    let target = address(10);
    let child = vec![PUSH1, 1, PUSH1, 1, TSTORE];
    let mut vm = Evm::new(&[
        PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 10, PUSH2, 0x27, 0x10, STATICCALL,
    ]);
    vm.state.base_state.insert(
        target,
        AccountState {
            code: child,
            ..AccountState::default()
        },
    );

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ZERO);
}

#[test]
fn test_transient_storage_cleared_after_tx() {
    let mut first = Evm::new(&[PUSH1, 7, PUSH1, 1, TSTORE, STOP]);
    first.run();
    assert!(first.state.transient_storage.is_empty());

    let mut second = Evm::new(&[PUSH1, 1, TLOAD, STOP]);
    second.state = first.state.clone();
    assert_eq!(second.run(), ExecutionResult::Halt);
    assert_eq!(second.stack.pop().unwrap(), U256::ZERO);
}

#[test]
fn transient_storage_helpers_use_address_and_key() {
    let first = address(1);
    let second = address(2);
    let mut state = StateFork {
        base_state: HashMap::new(),
        ..StateFork::default()
    };
    state.tstore(first, U256::from(3u8), U256::from(9u8));
    assert_eq!(state.tload(&first, &U256::from(3u8)), U256::from(9u8));
    assert_eq!(state.tload(&second, &U256::from(3u8)), U256::ZERO);
}
