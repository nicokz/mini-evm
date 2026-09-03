pub mod call_frame;
pub mod crypto;
pub mod disasm;
pub mod env;
pub mod gas;
pub mod log;
pub mod mpt;
pub mod opcodes;
pub mod precompiles;
pub mod rpc;
pub mod sim;
pub mod simulation;
pub mod stack;
pub mod state;
pub mod tracer;
pub mod vm;

#[cfg(test)]
mod tests {
    use super::opcodes::*;
    use super::vm::{Evm, ExecutionResult};
    use ruint::aliases::U256;

    #[test]
    fn test_push_add() {
        let bytecode = [PUSH1, 0x05, PUSH1, 0x07, ADD, STOP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.stack.pop().unwrap(), 12);
    }

    #[test]
    fn test_gas_consumption_and_out_of_gas() {
        let code = vec![PUSH1, 0x05, PUSH1, 0x07, ADD];

        let mut vm = Evm::new_with_gas(&code, 20);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.gas_left, 11);

        let mut vm_oog = Evm::new_with_gas(&code, 8);
        assert_eq!(vm_oog.run(), ExecutionResult::OutOfGas);
    }

    #[test]
    fn test_quadratic_memory_expansion_gas() {
        let code = vec![PUSH1, 0xFF, PUSH2, 0x04, 0x00, MSTORE8];

        // 3 + 3 + 3 base gas, plus 101 gas for expansion to 33 words.
        let mut vm = Evm::new_with_gas(&code, 110);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.gas_left, 0);
        assert_eq!(vm.memory.len(), 1056);

        let mut vm_oog = Evm::new_with_gas(&code, 109);
        assert_eq!(vm_oog.run(), ExecutionResult::OutOfGas);
    }

    #[test]
    fn test_underflow() {
        let bytecode = [ADD, STOP];
        let mut vm = Evm::new(&bytecode);
        assert!(matches!(vm.run(), ExecutionResult::Error(_)));
    }

    #[test]
    fn test_valid_and_invalid_jump() {
        // Offset 0: PUSH1 0x04
        // Offset 2: JUMP
        // Offset 3: STOP (should be skipped)
        // Offset 4: JUMPDEST
        // Offset 5: STOP
        let valid_bytecode = [PUSH1, 0x04, JUMP, STOP, JUMPDEST, STOP];
        let mut vm = Evm::new(&valid_bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);

        // Attempt jump to offset 0x03 (points to STOP, not JUMPDEST)
        let invalid_bytecode = [PUSH1, 0x03, JUMP, STOP, JUMPDEST, STOP];
        let mut vm = Evm::new(&invalid_bytecode);
        assert_eq!(vm.run(), ExecutionResult::Error("Invalid JUMP destination"));
    }

    #[test]
    fn test_memory_ops() {
        // Offset 0: MSIZE (stack: 0)
        // Offset 1: PUSH1 0x42 (val)
        // Offset 3: PUSH1 0x10 (offset = 16)
        // Offset 5: MSTORE (writes 32 bytes at offset 16 -> memory expands to 48 bytes [rounded 32-aligned])
        // Offset 6: MSIZE (stack: 0, 48)
        // Offset 7: PUSH1 0x10 (offset = 16)
        // Offset 9: MLOAD (reads back 0x42)
        let bytecode = [
            MSIZE, PUSH1, 0x42, PUSH1, 0x10, MSTORE, MSIZE, PUSH1, 0x10, MLOAD, STOP,
        ];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);

        assert_eq!(vm.stack.pop().unwrap(), 0x42); // Loaded value
        assert_eq!(vm.stack.pop().unwrap(), 64); // MSIZE after store at 16 (16 + 32 = 48 -> rounded to 64)
        assert_eq!(vm.stack.pop().unwrap(), 0); // MSIZE before store
    }

    #[test]
    fn test_dup_and_swap() {
        // Stack state (bottom -> top): [10, 10, 20]
        let bytecode = [PUSH1, 0x0A, PUSH1, 0x14, DUP2, SWAP1, STOP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);

        // Popping LIFO (top to bottom)
        assert_eq!(vm.stack.pop().unwrap(), 20);
        assert_eq!(vm.stack.pop().unwrap(), 10);
        assert_eq!(vm.stack.pop().unwrap(), 10);
    }

    #[test]
    fn test_comparison_ops() {
        // PUSH1 0x0A, PUSH1 0x0A, EQ (stack: 1), ISZERO (stack: 0), STOP
        let bytecode = [PUSH1, 0x0A, PUSH1, 0x0A, EQ, ISZERO, STOP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.stack.pop().unwrap(), 0);
    }

    #[test]
    fn test_storage_ops() {
        // PUSH1 42 (val), PUSH1 1 (key), SSTORE, PUSH1 1 (key), SLOAD, STOP
        let bytecode = [PUSH1, 42, PUSH1, 1, SSTORE, PUSH1, 1, SLOAD, STOP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.stack.pop().unwrap(), 42);
        assert_eq!(vm.storage.get(&U256::from(1u8)), Some(&U256::from(42u8)));
    }

    #[test]
    fn test_eip2929_warm_cold_sload() {
        let code = vec![PUSH1, 0x01, SLOAD, PUSH1, 0x01, SLOAD];

        let mut vm = Evm::new_with_gas(&code, 5_000);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.gas_left, 5_000 - 2_206);
        assert!(vm.accessed_slots.contains(&U256::from(1u8)));
    }

    #[test]
    fn test_calldatasize() {
        let code = vec![CALLDATASIZE];
        let mut evm = Evm::new(&code);
        evm.context.calldata = vec![0xaa, 0xbb, 0xcc, 0xdd];

        assert_eq!(evm.run(), ExecutionResult::Halt);
        assert_eq!(evm.stack.pop().unwrap(), 4);
    }

    #[test]
    fn test_calldataload_with_padding() {
        // PUSH1 0x02 -> CALLDATALOAD
        let code = vec![PUSH1, 0x02, CALLDATALOAD];
        let mut evm = Evm::new(&code);
        evm.context.calldata = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

        assert_eq!(evm.run(), ExecutionResult::Halt);
        // Offset 2 grabs [0x33, 0x44, 0x55, 0x66, 0x77, 0x88] right-zero-padded to 32 bytes.
        // Slicing top 8 bytes (MSB) yields 0x3344_5566_7788_0000.
        let val = evm.stack.pop().unwrap();
        let mut expected = [0u8; 32];
        expected[..6].copy_from_slice(&[0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
        assert_eq!(val, U256::from_be_bytes(expected));
    }

    #[test]
    fn test_calldatacopy_out_of_bounds_zero_fill() {
        // Copy 6 bytes from offset 2 of calldata into memory address 0
        // PUSH1 6 (size), PUSH1 2 (offset), PUSH1 0 (dest_offset), CALLDATACOPY
        let code = vec![PUSH1, 6, PUSH1, 2, PUSH1, 0, CALLDATACOPY];
        let mut evm = Evm::new(&code);
        evm.context.calldata = vec![0xde, 0xad, 0xbe, 0xef];

        assert_eq!(evm.run(), ExecutionResult::Halt);
        // Bytes 2 and 3 exist (0xbe, 0xef), remaining 4 bytes get padded with 0x00
        assert_eq!(&evm.memory[0..6], &[0xbe, 0xef, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_tx_context_opcodes() {
        // Bytecode: ADDRESS (0x30), CALLER (0x33), CALLVALUE (0x34)
        let code = vec![ADDRESS, CALLER, CALLVALUE];
        let mut evm = Evm::new(&code);

        let mut caller_addr = [0u8; 20];
        caller_addr[12..20].copy_from_slice(&0x1122334455667788u64.to_be_bytes());

        let mut contract_addr = [0u8; 20];
        contract_addr[12..20].copy_from_slice(&0xAABBCCDDEEFF0011u64.to_be_bytes());

        evm.context.caller = caller_addr;
        evm.context.address = contract_addr;
        evm.context.value = 1_000_000;

        assert_eq!(evm.run(), ExecutionResult::Halt);

        // Popped LIFO: CALLVALUE top, then CALLER, then ADDRESS
        assert_eq!(evm.stack.pop().unwrap(), 1_000_000);
        assert_eq!(evm.stack.pop().unwrap(), U256::from(0x1122334455667788u64));
        assert_eq!(evm.stack.pop().unwrap(), U256::from(0xAABBCCDDEEFF0011u64));
    }

    #[test]
    fn test_return_and_revert() {
        // PUSH1 0x42, PUSH1 0x00, MSTORE8, PUSH1 0x01 (size), PUSH1 0x00 (offset), RETURN
        let return_code = vec![
            PUSH1, 0x42, PUSH1, 0x00, MSTORE8, PUSH1, 0x01, PUSH1, 0x00, RETURN,
        ];
        let mut evm = Evm::new(&return_code);
        assert_eq!(evm.run(), ExecutionResult::Return(vec![0x42]));

        // Revert variant: return 2 bytes from memory offset 0
        let revert_code = vec![
            PUSH1, 0xEE, PUSH1, 0x00, MSTORE8, PUSH1, 0xFF, PUSH1, 0x01, MSTORE8, PUSH1, 0x02,
            PUSH1, 0x00, REVERT,
        ];
        let mut evm_revert = Evm::new(&revert_code);
        assert_eq!(evm_revert.run(), ExecutionResult::Revert(vec![0xEE, 0xFF]));
    }

    #[test]
    fn test_jump_into_push_payload_fails() {
        // Offset 0: PUSH1 0x5B (0x5B is JUMPDEST byte value, but it's data at offset 1)
        // Offset 2: PUSH1 0x01 (target offset 1)
        // Offset 4: JUMP (should fail because offset 1 is inside PUSH data)
        let bytecode = [PUSH1, JUMPDEST, PUSH1, 0x01, JUMP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Error("Invalid JUMP destination"));
    }
}
