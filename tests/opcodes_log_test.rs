use mini_evm::opcodes::*;
use mini_evm::vm::{Evm, ExecutionResult, VmError};
use ruint::aliases::U256;

fn push1(code: &mut Vec<u8>, value: u8) {
    code.extend_from_slice(&[PUSH1, value]);
}

#[test]
fn log0_through_log4_record_topics_and_memory() {
    for topic_count in 0..=4 {
        let mut code = Vec::new();
        push1(&mut code, 0xab);
        push1(&mut code, 0);
        code.push(MSTORE8);

        for topic in (1..=topic_count).rev() {
            push1(&mut code, topic as u8);
        }
        push1(&mut code, 1);
        push1(&mut code, 0);
        code.push(LOG0 + topic_count as u8);

        let mut vm = Evm::new(&code);
        vm.context.address[19] = 0x42;
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.logs.len(), 1);
        assert_eq!(vm.logs[0].address[19], 0x42);
        assert_eq!(
            vm.logs[0].topics,
            (1..=topic_count).map(U256::from).collect::<Vec<_>>()
        );
        assert_eq!(vm.logs[0].data, vec![0xab]);
    }
}

#[test]
fn log_memory_expansion_and_data_gas_are_charged() {
    let mut vm = Evm::new_with_gas(&[PUSH1, 2, PUSH1, 31, LOG0], 1_000);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.gas_left, 597);
    assert_eq!(vm.memory.len(), 64);
    assert_eq!(vm.logs[0].data, vec![0, 0]);
}

#[test]
fn logs_are_rejected_in_static_context() {
    let mut vm = Evm::new(&[PUSH1, 0, PUSH1, 0, LOG0]);
    vm.is_static = true;
    assert_eq!(
        vm.run(),
        ExecutionResult::VmError(VmError::StaticCallViolation)
    );
    assert!(vm.logs.is_empty());
}
