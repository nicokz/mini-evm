// src/vm/runner.rs

use primitive_types::{H256, U256};
use crate::state::StateFork;
use crate::tx::decoder::SignedTransaction;
use crate::vm::Vm;

#[derive(Debug, Clone)]
pub struct TxExecutionReceipt {
    pub tx_hash: H256,
    pub status: u64, // 1 = success, 0 = revert
    pub gas_used: u64,
    pub logs: Vec<u8>,
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
    let current_nonce = state.get_nonce(&tx.sender);
    if current_nonce != tx.nonce {
        return Err(ExecutionError::NonceMismatch {
            expected: current_nonce,
            got: tx.nonce,
        });
    }

    let max_gas_cost = U256::from(tx.gas_limit) * tx.max_fee_per_gas;
    let total_upfront_cost = tx.value + max_gas_cost;
    let sender_balance = state.get_balance(&tx.sender);

    if sender_balance < total_upfront_cost {
        return Err(ExecutionError::InsufficientBalance);
    }

    // Deduct maximum upfront gas cost & increment nonce
    state.set_balance(&tx.sender, sender_balance - max_gas_cost);
    state.set_nonce(&tx.sender, current_nonce + 1);

    // Execute state transition
    let mut vm = Vm::new(state);
    let vm_result = vm.execute_tx(
        tx.sender,
        tx.to,
        tx.value,
        &tx.data,
        tx.gas_limit,
    );

    let (status, gas_used) = match vm_result {
        Ok(res) if !res.reverted => (1u64, res.gas_used),
        Ok(res) => (0u64, res.gas_used),
        Err(_) => (0u64, tx.gas_limit), // Burn gas limit on internal error
    };

    // Refund unused gas
    let unused_gas = tx.gas_limit.saturating_sub(gas_used);
    let refund_amount = U256::from(unused_gas) * tx.max_fee_per_gas;
    let post_exec_balance = state.get_balance(&tx.sender);
    state.set_balance(&tx.sender, post_exec_balance + refund_amount);

    Ok(TxExecutionReceipt {
        tx_hash: tx.hash,
        status,
        gas_used,
        logs: vec![],
    })
}