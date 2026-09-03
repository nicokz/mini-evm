use super::types::{CallRequest, JsonRpcError};
use crate::gas::calculate_intrinsic_gas;
use crate::simulation::simulate_tx;
use crate::state::StateFork;
use ruint::aliases::U256;

pub fn estimate_transaction_gas(
    state: &StateFork,
    request: &CallRequest,
    block_gas_limit: u64,
) -> Result<u64, JsonRpcError> {
    let input = request
        .data
        .as_ref()
        .map(|bytes| bytes.0.as_slice())
        .unwrap_or(&[]);
    let intrinsic = calculate_intrinsic_gas(input, request.to.is_none());
    let explicit_limit = request.gas.map(|gas| gas.try_into());
    let mut high = match explicit_limit {
        Some(Ok(gas)) => gas,
        Some(Err(_)) => return Err(estimation_error("gas exceeds u64")),
        None => block_gas_limit,
    };
    if high < intrinsic {
        return Err(estimation_error("intrinsic gas exceeds gas limit"));
    }

    let target = request
        .to
        .ok_or_else(|| estimation_error("eth_estimateGas requires a to address"))?;
    let caller = request.from.unwrap_or([0; 20]);
    let value = request.value.unwrap_or(U256::ZERO);
    let succeeds = |total_gas: u64| {
        let execution_gas = total_gas - intrinsic;
        simulate_tx(state, caller, target, value, input, execution_gas).success
    };
    if !succeeds(high) {
        return Err(estimation_error("execution reverted during gas estimation"));
    }

    let mut low = intrinsic;
    while low < high {
        let mid = low + (high - low) / 2;
        if succeeds(mid) {
            high = mid;
        } else {
            low = mid.saturating_add(1);
        }
    }
    Ok(high)
}

fn estimation_error(message: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32000,
        message: message.into(),
        data: None,
    }
}
