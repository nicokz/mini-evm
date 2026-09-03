use mini_evm::crypto::{
    derive_create_address, derive_create2_address, keccak256, rlp_encode_create,
};
use mini_evm::opcodes::*;
use mini_evm::vm::{Evm, ExecutionResult, VmError};
use ruint::aliases::U256;

#[test]
fn create_rlp_encodes_sender_and_zero_nonce_minimally() {
    let sender = [0x11u8; 20];
    let mut expected = vec![0xd6, 0x94];
    expected.extend_from_slice(&sender);
    expected.push(0x80);
    assert_eq!(rlp_encode_create(&sender, 0), expected);
}

#[test]
fn test_create_deploys_runtime_bytecode() {
    let init_code = [0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x01, 0x60, 0x1f, 0xf3];
    let mut code = vec![PUSH10];
    code.extend_from_slice(&init_code);
    code.extend_from_slice(&[PUSH1, 0, MSTORE, PUSH1, 10, PUSH1, 22, PUSH1, 0, CREATE]);
    let mut vm = Evm::new(&code);
    let mut sender = [0u8; 20];
    sender[19] = 7;
    vm.context.address = sender;
    vm.storage_address = sender;

    assert_eq!(vm.run(), ExecutionResult::Halt);
    let created = derive_create_address(&sender, 0);
    assert_eq!(vm.stack.pop().unwrap(), {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(&created);
        U256::from_be_bytes(bytes)
    });
    assert_eq!(vm.contracts.get(&created), Some(&vec![1]));
    assert_eq!(vm.nonces.get(&sender), Some(&1));
}

#[test]
fn test_create2_deterministic_address() {
    let sender = [0x11u8; 20];
    let salt = U256::from(0x1234u16);
    let init_code = [0x60, 0x00, 0x60, 0x00, 0xf3];
    let actual = derive_create2_address(&sender, salt, &init_code);

    let mut preimage = vec![0xff];
    preimage.extend_from_slice(&sender);
    preimage.extend_from_slice(&salt.to_be_bytes::<32>());
    preimage.extend_from_slice(&keccak256(&init_code));
    let hash = keccak256(&preimage);
    let mut expected = [0u8; 20];
    expected.copy_from_slice(&hash[12..]);
    assert_eq!(actual, expected);
}

#[test]
fn test_staticcall_rejects_create() {
    let mut vm = Evm::new(&[CREATE]);
    vm.is_static = true;
    assert_eq!(
        vm.run(),
        ExecutionResult::VmError(VmError::StaticCallViolation)
    );
}

#[test]
fn test_create_code_deposit_out_of_gas() {
    let mut init_code = vec![PUSH32];
    init_code.extend_from_slice(&[0u8; 32]);
    init_code.extend_from_slice(&[PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN]);
    let source_offset = 19u8;
    let mut code = vec![
        PUSH1,
        init_code.len() as u8,
        PUSH1,
        source_offset,
        PUSH1,
        0,
        CODECOPY,
    ];
    code.extend_from_slice(&[
        PUSH1,
        init_code.len() as u8,
        PUSH1,
        0,
        PUSH1,
        0,
        CREATE,
        PUSH1,
        0,
        PUSH1,
        0,
        RETURN,
    ]);
    code.extend_from_slice(&init_code);
    let mut vm = Evm::new_with_gas(&code, 34_000);

    assert_eq!(vm.run(), ExecutionResult::Return(Vec::new()));
    assert_eq!(vm.stack.pop().unwrap(), U256::ZERO);
}
