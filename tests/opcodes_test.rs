use mini_evm::opcodes::*;
use mini_evm::stack::Stack;
use mini_evm::vm::{Evm, ExecutionResult, VmError};
use ruint::aliases::U256;

#[test]
fn push32_constructs_big_endian_word() {
    let mut code = vec![PUSH32];
    code.extend(1u8..=32);
    let mut vm = Evm::new(&code);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    let value = vm.stack.pop().unwrap();
    assert_eq!(value.as_limbs()[0], 0x191a1b1c1d1e1f20);
    assert_eq!(value.as_limbs()[3], 0x0102030405060708);
}

#[test]
fn truncated_push_zero_pads_immediate() {
    let mut vm = Evm::new(&[PUSH3, 0xaa]);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::from(0xaa0000u32));
    assert_eq!(vm.pc, 4);
}

#[test]
fn addmod_and_shl_use_u256() {
    let addmod_code = vec![PUSH1, 5, PUSH1, 3, PUSH1, 4, ADDMOD];
    let mut addmod_vm = Evm::new(&addmod_code);
    assert_eq!(addmod_vm.run(), ExecutionResult::Halt);
    assert_eq!(addmod_vm.stack.pop().unwrap(), U256::ZERO);

    let shl_code = vec![PUSH1, 1, PUSH1, 8, SHL];
    let mut shl_vm = Evm::new(&shl_code);
    assert_eq!(shl_vm.run(), ExecutionResult::Halt);
    assert_eq!(shl_vm.stack.pop().unwrap(), U256::from(256u16));
}

#[test]
fn keccak256_hashes_empty_memory() {
    let code = vec![PUSH1, 0, PUSH1, 0, KECCAK256];
    let mut vm = Evm::new(&code);
    let expected = U256::from_be_bytes([
        0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03,
        0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85,
        0xa4, 0x70,
    ]);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), expected);
}

#[test]
fn division_by_zero_and_stack_overflow_are_handled() {
    let code = vec![PUSH1, 7, PUSH1, 0, DIV];
    let mut vm = Evm::new(&code);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ZERO);

    let mut stack: Stack<1024> = Stack::new();
    for _ in 0..1024 {
        stack.push(U256::ONE).unwrap();
    }
    assert_eq!(stack.push(U256::ONE), Err("Stack Overflow"));
}

#[test]
fn non_commutative_opcodes_use_evm_operand_order() {
    let mut sub_vm = Evm::new(&[PUSH1, 5, PUSH1, 3, SUB]);
    assert_eq!(sub_vm.run(), ExecutionResult::Halt);
    assert_eq!(sub_vm.stack.pop().unwrap(), U256::from(2u8));

    let mut lt_vm = Evm::new(&[PUSH1, 2, PUSH1, 3, LT]);
    assert_eq!(lt_vm.run(), ExecutionResult::Halt);
    assert_eq!(lt_vm.stack.pop().unwrap(), U256::ONE);

    let mut gt_vm = Evm::new(&[PUSH1, 3, PUSH1, 2, GT]);
    assert_eq!(gt_vm.run(), ExecutionResult::Halt);
    assert_eq!(gt_vm.stack.pop().unwrap(), U256::ONE);
}

#[test]
fn memory_expansion_triggers_out_of_gas() {
    let code = vec![PUSH1, 0xff, PUSH2, 0x04, 0x00, MSTORE8];
    let mut vm = Evm::new_with_gas(&code, 109);
    assert_eq!(vm.run(), ExecutionResult::OutOfGas);
}

#[test]
fn environment_opcodes_push_environment_values() {
    let code = vec![
        GASPRICE, ORIGIN, COINBASE, TIMESTAMP, NUMBER, PREVRANDAO, GASLIMIT, CHAINID, BASEFEE,
    ];
    let mut vm = Evm::new(&code);
    vm.env.gas_price = U256::from(7u8);
    vm.env.origin[19] = 1;
    vm.env.coinbase[19] = 2;
    vm.env.timestamp = U256::from(3u8);
    vm.env.number = U256::from(4u8);
    vm.env.prevrandao = U256::from(5u8);
    vm.env.gas_limit = U256::from(6u8);
    vm.env.chain_id = U256::from(8u8);
    vm.env.base_fee = U256::from(9u8);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), 9);
    assert_eq!(vm.stack.pop().unwrap(), 8);
    assert_eq!(vm.stack.pop().unwrap(), 6);
    assert_eq!(vm.stack.pop().unwrap(), 5);
    assert_eq!(vm.stack.pop().unwrap(), 4);
    assert_eq!(vm.stack.pop().unwrap(), 3);
    assert_eq!(vm.stack.pop().unwrap(), 2);
    assert_eq!(vm.stack.pop().unwrap(), 1);
    assert_eq!(vm.stack.pop().unwrap(), 7);
}

#[test]
fn returndata_copy_checks_bounds_and_copies() {
    let mut vm = Evm::new(&[PUSH1, 2, PUSH1, 1, PUSH1, 0, RETURNDATACOPY]);
    vm.return_data = vec![0xaa, 0xbb, 0xcc];
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(&vm.memory[..2], &[0xbb, 0xcc]);

    let mut invalid = Evm::new(&[PUSH1, 2, PUSH1, 2, PUSH1, 0, RETURNDATACOPY]);
    invalid.return_data = vec![0xaa, 0xbb, 0xcc];
    assert_eq!(
        invalid.run(),
        ExecutionResult::VmError(VmError::OutOfBoundsReturnData)
    );
}

#[test]
fn log0_records_memory_data_and_static_calls_reject_logs() {
    let code = vec![PUSH1, 0x42, PUSH1, 0, MSTORE8, PUSH1, 1, PUSH1, 0, LOG0];
    let mut vm = Evm::new(&code);
    vm.context.address[19] = 9;
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.logs.len(), 1);
    assert_eq!(vm.logs[0].address[19], 9);
    assert_eq!(vm.logs[0].data, vec![0x42]);

    let mut static_vm = Evm::new(&[PUSH1, 0, PUSH1, 0, LOG0]);
    static_vm.is_static = true;
    assert_eq!(
        static_vm.run(),
        ExecutionResult::VmError(VmError::StaticCallViolation)
    );
}

#[test]
fn call_executes_registered_child_and_copies_return_data() {
    let child_code = vec![PUSH1, 0x2a, PUSH1, 0, MSTORE8, PUSH1, 1, PUSH1, 0, RETURN];
    let mut target = [0u8; 20];
    target[19] = 5;

    let parent_code = vec![
        PUSH1, 1, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 0, PUSH1, 5, PUSH2, 0x03, 0xe8, CALL,
    ];
    let mut vm = Evm::new(&parent_code);
    vm.contracts.insert(target, child_code);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ONE);
    assert_eq!(vm.return_data, vec![0x2a]);
    assert_eq!(vm.memory[0], 0x2a);
}
