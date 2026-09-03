use mini_evm::opcodes::{BLOBBASEFEE, BLOBHASH, PUSH1};
use mini_evm::vm::{Evm, EvmBuilder, ExecutionResult};
use ruint::aliases::U256;

#[test]
fn test_blobhash_in_bounds_and_out_of_bounds() {
    let hash1 = [0x01; 32];
    let hash2 = [0x02; 32];
    let mut evm = Evm::new_with_gas(
        &[
            PUSH1, 0x00, BLOBHASH, PUSH1, 0x01, BLOBHASH, PUSH1, 0x02, BLOBHASH,
        ],
        100,
    );
    evm.context.blob_versioned_hashes = vec![hash1, hash2];

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.values()[0], U256::from_be_bytes(hash1));
    assert_eq!(evm.stack.values()[1], U256::from_be_bytes(hash2));
    assert_eq!(evm.stack.values()[2], U256::ZERO);
    assert_eq!(evm.gas_left, 82);
}

#[test]
fn test_blobhash_large_index_returns_zero() {
    let mut evm = Evm::new(&[PUSH1, 0x01, BLOBHASH]);
    evm.context.blob_versioned_hashes = vec![[0xff; 32]];

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.pop().unwrap(), U256::ZERO);
}

#[test]
fn test_blobbasefee_reads_block_context() {
    let expected_fee = U256::from(100_000_000_000u64);
    let mut evm = Evm::new_with_gas(&[BLOBBASEFEE], 10);
    evm.env.blob_base_fee = expected_fee;

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.pop().unwrap(), expected_fee);
    assert_eq!(evm.gas_left, 8);
}

#[test]
fn test_builder_configures_blob_context() {
    let hash = [0xabu8; 32];
    let mut evm = EvmBuilder::new()
        .with_code([PUSH1, 0, BLOBHASH, BLOBBASEFEE])
        .with_blob_hashes(vec![hash])
        .with_blob_base_fee(U256::from(17u8))
        .build();

    assert_eq!(evm.run(), ExecutionResult::Halt);
    assert_eq!(evm.stack.values()[0], U256::from_be_bytes(hash));
    assert_eq!(evm.stack.values()[1], U256::from(17u8));
}
