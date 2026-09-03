use crate::env::Address;
use crate::log::LogRecord;
use crate::state::{AccountState, StateFork};
use crate::vm::{Evm, ExecutionResult};
use ruint::aliases::U256;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    pub state_diff: HashMap<Address, AccountState>,
    pub logs: Vec<LogRecord>,
}

pub fn simulate_tx(
    state: &StateFork,
    caller: Address,
    target: Address,
    value: U256,
    input: &[u8],
    gas_limit: u64,
) -> SimulationResult {
    let code = state.get_code(&target);
    let mut vm = Evm::new_with_gas(&code, gas_limit);
    vm.state = state.clone();
    vm.context.caller = caller;
    vm.context.address = target;
    vm.context.value = u128::try_from(value).unwrap_or(u128::MAX);
    vm.context.calldata = input.to_vec();
    vm.storage_address = target;
    let snapshot = vm.state.snapshot();
    let caller_balance = vm.state.get_balance(&caller);
    if caller_balance < value {
        return SimulationResult {
            success: false,
            gas_used: 0,
            return_data: Vec::new(),
            state_diff: HashMap::new(),
            logs: Vec::new(),
        };
    }
    vm.state.set_balance(caller, caller_balance - value);
    vm.state
        .set_balance(target, vm.state.get_balance(&target) + value);

    let result = vm.run();
    let success = matches!(result, ExecutionResult::Halt | ExecutionResult::Return(_));
    let return_data = match result {
        ExecutionResult::Return(data) | ExecutionResult::Revert(data) => data,
        _ => vm.return_data.clone(),
    };
    if !success {
        vm.state.revert_to_snapshot(snapshot);
    }

    SimulationResult {
        success,
        gas_used: gas_limit.saturating_sub(vm.gas_left),
        return_data,
        state_diff: vm.state.dirty_state,
        logs: vm.logs,
    }
}
