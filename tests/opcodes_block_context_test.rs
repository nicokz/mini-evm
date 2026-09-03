use mini_evm::opcodes::*;
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;

fn push_u16(code: &mut Vec<u8>, value: u16) {
    code.extend_from_slice(&[PUSH2, (value >> 8) as u8, value as u8]);
}

#[test]
fn block_context_opcodes_read_environment_values() {
    let mut vm = Evm::new(&[
        COINBASE, TIMESTAMP, NUMBER, PREVRANDAO, GASLIMIT, CHAINID, BASEFEE,
    ]);
    vm.env.coinbase = [0x11; 20];
    vm.env.timestamp = U256::from(2u8);
    vm.env.number = U256::from(3u8);
    vm.env.prevrandao = U256::from(4u8);
    vm.env.gas_limit = U256::from(5u8);
    vm.env.chain_id = U256::from(6u8);
    vm.env.base_fee = U256::from(7u8);

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop(), Ok(U256::from(7u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::from(6u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::from(5u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::from(4u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::from(3u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::from(2u8)));
    let coinbase = vm.stack.pop().unwrap().to_be_bytes::<32>();
    assert_eq!(&coinbase[12..], &[0x11; 20]);
}

#[test]
fn blockhash_returns_history_and_zero_for_invalid_ranges() {
    let mut code = Vec::new();
    code.extend_from_slice(&[PUSH1, 1, BLOCKHASH]);
    push_u16(&mut code, 0);
    code.push(BLOCKHASH);
    push_u16(&mut code, 257);
    code.push(BLOCKHASH);
    push_u16(&mut code, 256);
    code.extend_from_slice(&[BLOCKHASH, STOP]);

    let mut vm = Evm::new_with_gas(&code, 10_000);
    vm.env.number = U256::from(257u64);
    vm.env.block_hashes.insert(1, U256::from(0x11u8));
    vm.env.block_hashes.insert(0, U256::from(0x22u8));
    vm.env.block_hashes.insert(256, U256::from(0x44u8));

    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.gas_left, 9_908);
    assert_eq!(vm.stack.pop(), Ok(U256::from(0x44u8)));
    assert_eq!(vm.stack.pop(), Ok(U256::ZERO));
    assert_eq!(vm.stack.pop(), Ok(U256::ZERO));
    assert_eq!(vm.stack.pop(), Ok(U256::from(0x11u8)));
}

#[test]
fn blockhash_rejects_numbers_that_do_not_fit_u64() {
    let mut code = vec![PUSH32];
    code.extend([0xff; 32]);
    code.extend([BLOCKHASH, STOP]);
    let mut vm = Evm::new(&code);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop(), Ok(U256::ZERO));
}

#[test]
fn block_context_uses_default_zero_values() {
    let mut vm = Evm::new(&[COINBASE, PUSH1, 0, BLOCKHASH]);
    vm.env.number = U256::ZERO;
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.stack.pop(), Ok(U256::ZERO));
    let coinbase = vm.stack.pop().unwrap();
    assert_eq!(coinbase, U256::ZERO);
}
