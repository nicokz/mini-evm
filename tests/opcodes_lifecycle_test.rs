use mini_evm::opcodes::*;
use mini_evm::state::{AccountState, StateFork};
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;

fn address(last_byte: u8) -> [u8; 20] {
    let mut address = [0u8; 20];
    address[19] = last_byte;
    address
}

fn push_address(code: &mut Vec<u8>, address: [u8; 20]) {
    code.push(PUSH20);
    code.extend_from_slice(&address);
}

#[test]
fn pre_existing_selfdestruct_transfers_balance_and_preserves_state() {
    let contract = address(1);
    let beneficiary = address(2);
    let mut state = StateFork::default();
    let mut storage = std::collections::HashMap::new();
    storage.insert(U256::from(1u8), U256::from(42u8));
    state.base_state.insert(
        contract,
        AccountState {
            balance: U256::from(100u8),
            code: vec![PUSH1, beneficiary[19], SELFDESTRUCT],
            storage,
            ..AccountState::default()
        },
    );

    let mut vm = Evm::new_with_gas(
        &[
            PUSH20,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            SELFDESTRUCT,
        ],
        20_000,
    );
    vm.code.clear();
    push_address(&mut vm.code, beneficiary);
    vm.code.push(SELFDESTRUCT);
    vm.context.address = contract;
    vm.state = state;

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.state.get_balance(&beneficiary), U256::from(100u8));
    assert_eq!(vm.state.get_balance(&contract), U256::ZERO);
    assert_eq!(
        vm.state.get_code(&contract),
        vec![PUSH1, beneficiary[19], SELFDESTRUCT]
    );
    assert_eq!(
        vm.state.get_storage(&contract, U256::from(1u8)),
        U256::from(42u8)
    );
}

#[test]
fn same_transaction_selfdestruct_removes_created_account() {
    let contract = address(3);
    let beneficiary = address(4);
    let mut vm = Evm::new_with_gas(&[], 20_000);
    push_address(&mut vm.code, beneficiary);
    vm.code.push(SELFDESTRUCT);
    vm.context.address = contract;
    vm.state.set_balance(contract, U256::from(100u8));
    vm.state.set_code(contract, vec![PUSH1, 1, STOP]);
    vm.state
        .set_storage(contract, U256::from(1u8), U256::from(7u8));
    vm.created_in_transaction.insert(contract);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.state.get_balance(&beneficiary), U256::from(100u8));
    assert_eq!(vm.state.get_account(&contract), AccountState::default());
}

#[test]
fn create_then_call_selfdestruct_removes_account_in_same_transaction() {
    let creator = address(7);
    let beneficiary = address(8);
    let runtime = {
        let mut code = Vec::new();
        push_address(&mut code, beneficiary);
        code.push(SELFDESTRUCT);
        code
    };
    let mut init = vec![PUSH32];
    let mut runtime_word = [0u8; 32];
    runtime_word[..runtime.len()].copy_from_slice(&runtime);
    init.extend_from_slice(&runtime_word);
    init.extend_from_slice(&[PUSH1, runtime.len() as u8, PUSH1, 0, RETURN]);

    let mut vm = Evm::new_with_gas(&init, 100_000);
    vm.context.address = creator;
    vm.state.set_balance(creator, U256::from(100u8));
    vm.balances.insert(creator, U256::from(100u8));
    let created_address = address(9);
    vm.execute_deploy(created_address, U256::ZERO, &init, 50_000)
        .unwrap();
    vm.contracts.insert(created_address, runtime.clone());
    vm.state.set_code(created_address, runtime);
    vm.code.clear();
    for _ in 0..5 {
        vm.code.extend_from_slice(&[PUSH1, 0]);
    }
    push_address(&mut vm.code, created_address);
    vm.code.extend_from_slice(&[PUSH2, 0xc3, 0x50, CALL]);
    vm.pc = 0;

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop(), Ok(U256::ONE));
    assert_eq!(
        vm.state.get_account(&created_address),
        AccountState::default()
    );
}

#[test]
fn selfdestruct_in_staticcall_returns_static_violation() {
    let target = address(5);
    let beneficiary = address(6);
    let mut child_code = Vec::new();
    push_address(&mut child_code, beneficiary);
    child_code.push(SELFDESTRUCT);

    let mut vm = Evm::new_with_gas(
        &[
            PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, STATICCALL,
        ],
        50_000,
    );
    vm.code.clear();
    push1_args_for_staticcall(&mut vm.code, target);
    vm.code.push(STATICCALL);
    vm.state.set_code(target, child_code);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop(), Ok(U256::ZERO));
}

fn push1_args_for_staticcall(code: &mut Vec<u8>, target: [u8; 20]) {
    code.extend_from_slice(&[PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0]);
    push_address(code, target);
    code.extend_from_slice(&[PUSH2, 0x27, 0x10]);
}

#[test]
fn invalid_opcode_consumes_all_remaining_gas() {
    let mut vm = Evm::new_with_gas(&[INVALID], 10_000);
    assert_eq!(vm.run(), ExecutionResult::InvalidOpcode(INVALID));
    assert_eq!(vm.gas_left, 0);
}
