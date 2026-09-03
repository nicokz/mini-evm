use crate::fixture_types::StateTestCase;
use k256::ecdsa::SigningKey;
use mini_evm::gas::calc_data_fee;
use mini_evm::state::{AccountState, StateFork};
use mini_evm::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug)]
pub enum RunnerError {
    MissingTransactionField(&'static str),
    InvalidFork(String),
    InsufficientBalance,
    Execution(ExecutionResult),
}

pub fn run_state_test_case(case: &StateTestCase, fork: &str) -> Result<[u8; 32], RunnerError> {
    let data_index = case
        .post
        .get(fork)
        .and_then(|_| case.transaction.data.first().map(|_| 0))
        .ok_or_else(|| RunnerError::InvalidFork(fork.to_owned()))?;
    let target = case
        .transaction
        .to
        .ok_or(RunnerError::MissingTransactionField("to"))?;
    let sender = case
        .transaction
        .sender
        .or_else(|| {
            case.transaction
                .secret_key
                .as_deref()
                .and_then(derive_sender)
        })
        .unwrap_or([0; 20]);
    let gas_limit = *case
        .transaction
        .gas_limit
        .get(data_index)
        .ok_or(RunnerError::MissingTransactionField("gasLimit"))?;
    let gas_price = *case
        .transaction
        .gas_price
        .get(data_index)
        .ok_or(RunnerError::MissingTransactionField("gasPrice"))?;
    let value = *case
        .transaction
        .value
        .get(data_index)
        .ok_or(RunnerError::MissingTransactionField("value"))?;
    let input = case
        .transaction
        .data
        .get(data_index)
        .ok_or(RunnerError::MissingTransactionField("data"))?;

    let mut state = StateFork::default();
    let mut storage_by_address = HashMap::new();
    for (address, account) in &case.pre {
        state.base_state.insert(
            *address,
            AccountState {
                nonce: account.nonce,
                balance: account.balance,
                code: account.code.clone(),
                storage: account.storage.clone(),
            },
        );
        storage_by_address.insert(*address, account.storage.clone());
    }

    let code = state.get_code(&target);
    let mut vm = Evm::new_with_gas(&code, u64_from_u256(gas_limit)?);
    vm.state = state;
    vm.context.caller = sender;
    vm.context.address = target;
    vm.context.value = u128::try_from(value).unwrap_or(u128::MAX);
    vm.context.calldata = input.clone();
    vm.storage_address = target;
    vm.env.coinbase = case.env.current_coinbase;
    vm.env.gas_limit = case.env.current_gas_limit;
    vm.env.number = case.env.current_number;
    vm.env.timestamp = case.env.current_timestamp;
    vm.env.base_fee = case.env.current_base_fee.unwrap_or(U256::ZERO);
    vm.env.prevrandao = case.env.current_random.unwrap_or(U256::ZERO);
    vm.env.blob_base_fee = case
        .env
        .current_excess_blob_gas
        .map(calc_data_fee)
        .unwrap_or(U256::ZERO);

    if let Some(hashes) = &case.transaction.blob_versioned_hashes {
        vm.context.blob_versioned_hashes = hashes
            .iter()
            .map(|hash| {
                hash.as_slice()
                    .try_into()
                    .map_err(|_| RunnerError::InvalidFork("blob hash must be 32 bytes".into()))
            })
            .collect::<Result<_, _>>()?;
    }

    for (address, account) in &case.pre {
        vm.balances.insert(*address, account.balance);
        vm.nonces.insert(*address, account.nonce);
        vm.contracts.insert(*address, account.code.clone());
    }
    vm.storage = storage_by_address.remove(&target).unwrap_or_default();

    let gas_cost = gas_limit
        .checked_mul(gas_price)
        .ok_or(RunnerError::Execution(ExecutionResult::VmError(
            mini_evm::vm::VmError::Overflow,
        )))?;
    let sender_balance = vm.balances.get(&sender).copied().unwrap_or(U256::ZERO);
    if sender_balance < gas_cost {
        return Err(RunnerError::InsufficientBalance);
    }
    vm.balances.insert(sender, sender_balance - gas_cost);
    vm.nonces.insert(
        sender,
        vm.nonces
            .get(&sender)
            .copied()
            .unwrap_or(0)
            .saturating_add(1),
    );

    match vm.run() {
        ExecutionResult::Halt | ExecutionResult::Return(_) | ExecutionResult::Revert(_) => {
            Ok(vm.state.compute_state_root())
        }
        result => Err(RunnerError::Execution(result)),
    }
}

fn u64_from_u256(value: U256) -> Result<u64, RunnerError> {
    value
        .try_into()
        .map_err(|_| RunnerError::InvalidFork("value exceeds u64".into()))
}

fn derive_sender(secret_key: &str) -> Option<[u8; 20]> {
    let bytes = hex::decode(secret_key.strip_prefix("0x").unwrap_or(secret_key)).ok()?;
    let signing_key = SigningKey::from_slice(&bytes).ok()?;
    let public_key = signing_key.verifying_key().to_encoded_point(false);
    let hash = mini_evm::crypto::keccak256(&public_key.as_bytes()[1..]);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    Some(address)
}
