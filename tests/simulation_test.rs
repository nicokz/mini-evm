use mini_evm::env::Address;
use mini_evm::opcodes::*;
use mini_evm::simulation::simulate_tx;
use mini_evm::state::{AccountState, StateFork};
use ruint::aliases::U256;
use std::collections::HashMap;

fn address(last_byte: u8) -> Address {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

#[test]
fn test_speculative_execution_rollback() {
    let target = address(1);
    let mut state = StateFork::default();
    state.base_state.insert(
        target,
        AccountState {
            code: vec![PUSH1, 42, PUSH1, 1, SSTORE, PUSH1, 0, PUSH1, 0, REVERT],
            ..AccountState::default()
        },
    );

    let result = simulate_tx(&state, address(2), target, U256::ZERO, &[], 100_000);

    assert!(!result.success);
    assert!(result.state_diff.is_empty());
    assert_eq!(state.get_storage(&target, U256::from(1u8)), U256::ZERO);
}

#[test]
fn test_nested_snapshots() {
    let target = address(1);
    let mut state = StateFork::default();
    state.set_storage(target, U256::from(1u8), U256::from(10u8));
    let first = state.snapshot();
    state.set_storage(target, U256::from(1u8), U256::from(20u8));
    let second = state.snapshot();
    state.set_storage(target, U256::from(1u8), U256::from(30u8));

    state.revert_to_snapshot(second);
    assert_eq!(
        state.get_storage(&target, U256::from(1u8)),
        U256::from(20u8)
    );
    state.revert_to_snapshot(first);
    assert_eq!(
        state.get_storage(&target, U256::from(1u8)),
        U256::from(10u8)
    );
}

#[test]
fn test_state_diff_extraction() {
    let caller = address(2);
    let target = address(1);
    let state = StateFork {
        base_state: HashMap::from([
            (
                caller,
                AccountState {
                    balance: U256::from(100u8),
                    ..AccountState::default()
                },
            ),
            (
                target,
                AccountState {
                    code: vec![PUSH1, 7, PUSH1, 3, SSTORE],
                    ..AccountState::default()
                },
            ),
        ]),
        ..StateFork::default()
    };

    let result = simulate_tx(&state, caller, target, U256::from(9u8), &[], 100_000);

    assert!(result.success);
    assert_eq!(
        result.state_diff[&target].storage[&U256::from(3u8)],
        U256::from(7u8)
    );
    assert_eq!(result.state_diff[&caller].balance, U256::from(91u8));
    assert_eq!(result.state_diff[&target].balance, U256::from(9u8));
    assert!(state.dirty_state.is_empty());
}
