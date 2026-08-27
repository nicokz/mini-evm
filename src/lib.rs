pub mod opcodes;
pub mod stack;
pub mod vm;

#[cfg(test)]
mod tests {
    use super::opcodes::*;
    use super::vm::{Evm, ExecutionResult};

    #[test]
    fn test_push_add() {
        let bytecode = [PUSH1, 0x05, PUSH1, 0x07, ADD, STOP];
        let mut vm = Evm::new(&bytecode);
        assert_eq!(vm.run(), ExecutionResult::Halt);
        assert_eq!(vm.stack.pop().unwrap(), 12);
    }

    #[test]
    fn test_underflow() {
        let bytecode = [ADD, STOP];
        let mut vm = Evm::new(&bytecode);
        assert!(matches!(vm.run(), ExecutionResult::Revert(_)));
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
        assert_eq!(vm.run(), ExecutionResult::Revert("Invalid JUMP destination"));
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
        assert_eq!(vm.stack.pop().unwrap(), 64);   // MSIZE after store at 16 (16 + 32 = 48 -> rounded to 64)
        assert_eq!(vm.stack.pop().unwrap(), 0);    // MSIZE before store
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

}