use mini_evm::env::Address;
use mini_evm::opcodes::*;
use mini_evm::precompiles::execute_precompile;
use mini_evm::vm::{Evm, ExecutionResult, VmError};
use ruint::aliases::U256;
use sha2::{Digest, Sha256};

fn address(number: u8) -> Address {
    let mut result = [0u8; 20];
    result[19] = number;
    result
}

#[test]
fn test_sha256_word_gas() {
    let precompile = address(2);
    assert_eq!(execute_precompile(&precompile, &[], 60).unwrap().0, 60);
    assert_eq!(execute_precompile(&precompile, &[0; 32], 72).unwrap().0, 72);
    assert_eq!(execute_precompile(&precompile, &[0; 33], 84).unwrap().0, 84);
    assert_eq!(
        execute_precompile(&precompile, &[0; 33], 83),
        Err(VmError::OutOfGas)
    );
}

#[test]
fn test_ripemd160_padding() {
    let (gas, output) = execute_precompile(&address(3), b"hello", 1_000).unwrap();
    assert_eq!(gas, 720);
    assert_eq!(output.len(), 32);
    assert_eq!(&output[..12], &[0; 12]);
}

#[test]
fn test_identity_copy() {
    let input = [0xde, 0xad, 0xbe, 0xef];
    let (gas, output) = execute_precompile(&address(4), &input, 18).unwrap();
    assert_eq!(gas, 18);
    assert_eq!(output, input);
}

#[test]
fn test_ecrecover_invalid_signature_returns_zero_word() {
    let (gas, output) = execute_precompile(&address(1), &[], 3_000).unwrap();
    assert_eq!(gas, 3_000);
    assert_eq!(output, vec![0; 32]);
}

#[test]
fn test_sha256_output_matches_digest() {
    let input = b"precompile";
    let (_, output) = execute_precompile(&address(2), input, 1_000).unwrap();
    assert_eq!(output, Sha256::digest(input).to_vec());
}

#[test]
fn identity_precompile_is_reachable_through_call() {
    let code = vec![
        PUSH1, 0xef, PUSH1, 0, MSTORE8, PUSH1, 1, PUSH1, 1, PUSH1, 1, PUSH1, 0, PUSH1, 0, PUSH1, 4,
        PUSH2, 0x03, 0xe8, CALL,
    ];
    let mut vm = Evm::new(&code);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop().unwrap(), U256::ONE);
    assert_eq!(vm.return_data, vec![0xef]);
    assert_eq!(vm.memory[1], 0xef);
}
