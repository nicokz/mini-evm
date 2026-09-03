use mini_evm::gas::calc_eip150_call_gas;
use mini_evm::opcodes::*;
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;

#[test]
fn requested_gas_is_capped_at_sixty_three_over_sixty_fourths() {
    assert_eq!(calc_eip150_call_gas(64, U256::from(100u16)), 63);
    assert_eq!(calc_eip150_call_gas(1_000, U256::MAX), 985);
}

#[test]
fn requested_gas_below_cap_is_preserved() {
    assert_eq!(calc_eip150_call_gas(1_000, U256::from(500u16)), 500);
    assert_eq!(calc_eip150_call_gas(64, U256::ZERO), 0);
}

#[test]
fn unused_child_gas_is_refunded() {
    let mut vm = Evm::new_with_gas(
        &[
            PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 5, PUSH2, 0x03, 0xe8, CALL,
        ],
        10_000,
    );
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ONE);
    assert_eq!(vm.gas_left, 6_679);
}
