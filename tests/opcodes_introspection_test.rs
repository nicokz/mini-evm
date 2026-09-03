use mini_evm::env::Address;
use mini_evm::opcodes::*;
use mini_evm::state::{AccountState, StateFork};
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;
use std::collections::HashMap;

fn address(last_byte: u8) -> Address {
    let mut address = [0u8; 20];
    address[19] = last_byte;
    address
}

fn push_address(code: &mut Vec<u8>, address: Address) {
    code.push(PUSH20);
    code.extend_from_slice(&address);
}

fn state_with_account(address: Address, account: AccountState) -> StateFork {
    let mut state = StateFork::default();
    state.base_state.insert(address, account);
    state
}

#[test]
fn balance_and_selfbalance_read_state_accounts() {
    let target = address(1);
    let context = address(2);
    let mut code = Vec::new();
    push_address(&mut code, target);
    code.extend_from_slice(&[BALANCE, STOP]);
    let mut evm = Evm::new(&code);
    evm.context.address = context;
    evm.state = state_with_account(
        target,
        AccountState {
            balance: U256::from(1234u64),
            ..AccountState::default()
        },
    );
    evm.state.set_balance(context, U256::from(5678u64));
    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.pop(), Ok(U256::from(1234u64)));

    let mut selfbalance = Evm::new(&[SELFBALANCE, STOP]);
    selfbalance.context.address = context;
    selfbalance.state = evm.state;
    assert_eq!(selfbalance.run(), ExecutionResult::Halt);
    assert_eq!(selfbalance.stack.pop(), Ok(U256::from(5678u64)));
}

#[test]
fn extcodesize_and_extcodehash_read_target_code() {
    let target = address(3);
    let target_code = vec![PUSH1, 0xaa, STOP];
    let mut code = Vec::new();
    push_address(&mut code, target);
    code.push(EXTCODESIZE);
    push_address(&mut code, target);
    code.extend_from_slice(&[EXTCODEHASH, STOP]);
    let mut evm = Evm::new(&code);
    evm.state = state_with_account(
        target,
        AccountState {
            code: target_code.clone(),
            ..AccountState::default()
        },
    );
    assert_eq!(evm.run(), ExecutionResult::Halt);
    let mut expected_hash = [0u8; 32];
    expected_hash.copy_from_slice(&mini_evm::crypto::keccak256(&target_code));
    assert_eq!(evm.stack.pop(), Ok(U256::from_be_bytes(expected_hash)));
    assert_eq!(evm.stack.pop(), Ok(U256::from(target_code.len())));
}

#[test]
fn extcodecopy_copies_code_and_zero_pads() {
    let target = address(4);
    let mut code = vec![PUSH1, 5, PUSH1, 1, PUSH1, 0];
    push_address(&mut code, target);
    code.extend_from_slice(&[EXTCODECOPY, STOP]);
    let mut evm = Evm::new(&code);
    evm.state = state_with_account(
        target,
        AccountState {
            code: vec![0xaa, 0xbb, 0xcc],
            ..AccountState::default()
        },
    );
    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(&evm.memory[..5], &[0xbb, 0xcc, 0, 0, 0]);
}

#[test]
fn account_introspection_uses_warm_access_cost_after_first_touch() {
    let target = address(5);
    let mut code = Vec::new();
    push_address(&mut code, target);
    code.push(BALANCE);
    push_address(&mut code, target);
    code.extend_from_slice(&[BALANCE, STOP]);
    let mut evm = Evm::new_with_gas(&code, 10_000);
    evm.state = state_with_account(target, AccountState::default());
    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.gas_left, 7294);
    assert!(evm.accessed_addresses.contains(&target));
}

#[test]
fn introspection_accepts_zero_and_empty_accounts() {
    let target = address(6);
    let mut accounts = HashMap::new();
    accounts.insert(target, AccountState::default());
    let mut code = Vec::new();
    push_address(&mut code, target);
    code.extend_from_slice(&[EXTCODESIZE, STOP]);
    let mut evm = Evm::new(&code);
    evm.state = StateFork {
        base_state: accounts,
        ..StateFork::default()
    };
    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.pop(), Ok(U256::ZERO));
}
