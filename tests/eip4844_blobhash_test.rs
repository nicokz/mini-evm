use mini_evm::opcodes::{BLOBHASH, PUSH1, PUSH32};
use mini_evm::vm::{EvmBuilder, ExecutionResult};
use ruint::aliases::U256;

fn make_blobhash_bytecode(index: U256) -> Vec<u8> {
    let mut code = Vec::with_capacity(34);
    code.push(PUSH32);
    code.extend_from_slice(&index.to_be_bytes::<32>());
    code.push(BLOBHASH);
    code
}

#[test]
fn test_blobhash_empty_array_always_returns_zero() {
    let code = vec![PUSH1, 0x00, BLOBHASH, PUSH1, 0x01, BLOBHASH];
    let mut evm = EvmBuilder::new()
        .with_code(code)
        .with_blob_hashes(vec![])
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.values().len(), 2);
    assert_eq!(evm.stack.values()[0], U256::ZERO);
    assert_eq!(evm.stack.values()[1], U256::ZERO);
}

#[test]
fn test_blobhash_out_of_order_lookups() {
    let hash_0 = [0xAA; 32];
    let hash_1 = [0xBB; 32];
    let hash_2 = [0xCC; 32];
    let code = vec![
        PUSH1, 0x02, BLOBHASH, PUSH1, 0x00, BLOBHASH, PUSH1, 0x01, BLOBHASH,
    ];
    let mut evm = EvmBuilder::new()
        .with_code(code)
        .with_blob_hashes(vec![hash_0, hash_1, hash_2])
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    let stack = evm.stack.values();
    assert_eq!(stack[0], U256::from_be_bytes(hash_2));
    assert_eq!(stack[1], U256::from_be_bytes(hash_0));
    assert_eq!(stack[2], U256::from_be_bytes(hash_1));
}

#[test]
fn test_blobhash_boundary_and_out_of_bounds() {
    let hash_0 = [0x11; 32];
    let hash_1 = [0x22; 32];
    let code = vec![PUSH1, 0x01, BLOBHASH, PUSH1, 0x02, BLOBHASH];
    let mut evm = EvmBuilder::new()
        .with_code(code)
        .with_blob_hashes(vec![hash_0, hash_1])
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    let stack = evm.stack.values();
    assert_eq!(stack[0], U256::from_be_bytes(hash_1));
    assert_eq!(stack[1], U256::ZERO);
}

#[test]
fn test_blobhash_max_usize_and_u256_overflow() {
    let hash_0 = [0xFF; 32];
    let mut evm = EvmBuilder::new()
        .with_blob_hashes(vec![hash_0])
        .with_code(make_blobhash_bytecode(U256::from(usize::MAX)))
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.values()[0], U256::ZERO);

    let mut evm = EvmBuilder::new()
        .with_blob_hashes(vec![hash_0])
        .with_code(make_blobhash_bytecode(U256::MAX))
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.values()[0], U256::ZERO);
}
