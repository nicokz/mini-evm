// src/vm/runner.rs

use crate::env::Address;
use crate::log::LogRecord;
use crate::state::StateFork;
use crate::tx::decoder::SignedTransaction;
use crate::vm::{Evm, ExecutionResult};
use primitive_types::H256;
use ruint::aliases::U256;

#[derive(Debug, Clone)]
pub struct TxExecutionReceipt {
    pub tx_hash: H256,
    pub status: u64, // 1 = success, 0 = revert
    pub gas_used: u64,
    pub logs: Vec<LogRecord>,
}

#[derive(Debug)]
pub enum ExecutionError {
    NonceMismatch { expected: u64, got: u64 },
    InsufficientBalance,
    VmExecutionError(String),
}

pub fn apply_signed_transaction(
    state: &mut StateFork,
    tx: &SignedTransaction,
) -> Result<TxExecutionReceipt, ExecutionError> {
    let sender: Address = *tx.sender.as_fixed_bytes();
    let current_nonce = state.get_account(&sender).nonce;
    if current_nonce != tx.nonce {
        return Err(ExecutionError::NonceMismatch {
            expected: current_nonce,
            got: tx.nonce,
        });
    }

    let mut value_bytes = [0u8; 32];
    tx.value.to_big_endian(&mut value_bytes);
    let value = U256::from_be_bytes(value_bytes);
    let mut max_fee_bytes = [0u8; 32];
    tx.max_fee_per_gas.to_big_endian(&mut max_fee_bytes);
    let max_fee_per_gas = U256::from_be_bytes(max_fee_bytes);
    let max_gas_cost = U256::from(tx.gas_limit) * max_fee_per_gas;
    let total_upfront_cost = value + max_gas_cost;
    let sender_balance = state.get_balance(&sender);

    if sender_balance < total_upfront_cost {
        return Err(ExecutionError::InsufficientBalance);
    }

    // Deduct maximum upfront gas cost & increment nonce
    state.set_balance(sender, sender_balance - max_gas_cost);
    state.set_nonce(sender, current_nonce + 1);

    // Execute state transition
    let target = tx.to.map(|address| *address.as_fixed_bytes());
    let code = target
        .map(|address| state.get_code(&address))
        .unwrap_or_default();
    let mut vm = Evm::new_with_gas(&code, tx.gas_limit);
    vm.state = state.clone();
    vm.context.caller = sender;
    vm.context.address = target.unwrap_or([0; 20]);
    vm.context.value = u128::try_from(value).unwrap_or(u128::MAX);
    vm.context.calldata = tx.data.clone();
    vm.storage_address = target.unwrap_or([0; 20]);

    let result = vm.run();
    let (status, gas_used) = match result {
        ExecutionResult::Halt | ExecutionResult::Return(_) => {
            *state = vm.state;
            (1u64, tx.gas_limit.saturating_sub(vm.gas_left))
        }
        _ => (0u64, tx.gas_limit.saturating_sub(vm.gas_left)),
    };

    // Refund unused gas
    let unused_gas = tx.gas_limit.saturating_sub(gas_used);
    let refund_amount = U256::from(unused_gas) * max_fee_per_gas;
    let post_exec_balance = state.get_balance(&sender);
    state.set_balance(sender, post_exec_balance + refund_amount);

    let logs = if status == 1 { vm.logs } else { Vec::new() };
    Ok(TxExecutionReceipt {
        tx_hash: tx.hash,
        status,
        gas_used,
        logs,
    })
}
