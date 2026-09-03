use super::RpcState;
use super::types::{CallRequest, JsonRpcError, decode_address, decode_u256};
use crate::simulation::simulate_tx;
use crate::tx::decoder::decode_raw_tx;
use crate::vm::runner::apply_signed_transaction; // or crate::vm::runner depending on where runner.rs lives
use ruint::aliases::U256;
use serde_json::Value;

pub async fn handle_eth_call(state: &RpcState, params: Value) -> Result<String, JsonRpcError> {
    let call = parse_call_request(params)?;
    let target = call
        .to
        .ok_or_else(|| invalid_params("eth_call requires a to address"))?;
    let caller = call.from.unwrap_or([0; 20]);
    let gas_limit = call
        .gas
        .unwrap_or(U256::from(30_000_000u64))
        .try_into()
        .map_err(|_| invalid_params("gas exceeds u64"))?;
    let value = call.value.unwrap_or(U256::ZERO);
    let data = call.data.map(|bytes| bytes.0).unwrap_or_default();
    let state = state.state.read().await;
    let result = simulate_tx(&state, caller, target, value, &data, gas_limit);
    if !result.success {
        return Err(JsonRpcError {
            code: -32000,
            message: if result.return_data.is_empty() {
                "execution reverted".into()
            } else {
                format!("execution reverted: 0x{}", hex::encode(result.return_data))
            },
            data: None,
        });
    }
    Ok(format!("0x{}", hex::encode(result.return_data)))
}

pub async fn handle_eth_estimate_gas(
    state: &RpcState,
    params: Value,
) -> Result<String, JsonRpcError> {
    let call = parse_call_request(params)?;
    let state = state.state.read().await;
    let gas = super::estimator::estimate_transaction_gas(&state, &call, 30_000_000)?;
    Ok(format!("0x{gas:x}"))
}

pub async fn handle_eth_get_storage_at(
    state: &RpcState,
    params: Value,
) -> Result<String, JsonRpcError> {
    let values = params
        .as_array()
        .ok_or_else(|| invalid_params("eth_getStorageAt params must be an array"))?;
    if values.len() < 2 {
        return Err(invalid_params(
            "eth_getStorageAt requires address and position",
        ));
    }
    let address = decode_address(
        values[0]
            .as_str()
            .ok_or_else(|| invalid_params("invalid address"))?,
    )
    .map_err(|error| invalid_params(&error))?;
    let slot = decode_u256(
        values[1]
            .as_str()
            .ok_or_else(|| invalid_params("invalid storage position"))?,
    )
    .map_err(|error| invalid_params(&error))?;
    let state = state.state.read().await;
    let value = state.get_storage_at(&address, &slot).to_be_bytes::<32>();
    Ok(format!("0x{}", hex::encode(value)))
}

pub async fn handle_eth_get_code(state: &RpcState, params: Value) -> Result<String, JsonRpcError> {
    let values = params
        .as_array()
        .ok_or_else(|| invalid_params("eth_getCode params must be an array"))?;
    let address_value = values
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("eth_getCode requires an address"))?;
    let address = decode_address(address_value).map_err(|error| invalid_params(&error))?;
    let state = state.state.read().await;
    Ok(format!("0x{}", hex::encode(state.get_code(&address))))
}

pub async fn handle_eth_send_raw_transaction(
    state: &RpcState,
    params: Value,
) -> Result<String, JsonRpcError> {
    let raw = params
        .as_array()
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| JsonRpcError::invalid_params("eth_sendRawTransaction requires a raw transaction"))?;

    let bytes = hex::decode(raw.strip_prefix("0x").unwrap_or(raw))
        .map_err(|_| JsonRpcError::invalid_params("Invalid raw transaction hex"))?;

    let transaction = decode_raw_tx(&bytes).map_err(|e| {
        JsonRpcError::custom(-32000, format!("Invalid signature or transaction encoding: {e:?}"))
    })?;

    let mut state_guard = state.state.write().await;
    let receipt = apply_signed_transaction(&mut state_guard, &transaction).map_err(|error| {
        JsonRpcError::custom(-32000, format!("transaction execution failed: {error:?}"))
    })?;
    drop(state_guard);

    state
        .receipts
        .write()
        .await
        .insert(transaction.hash, receipt);

    Ok(format!("0x{}", hex::encode(transaction.hash)))
}

fn parse_call_request(params: Value) -> Result<CallRequest, JsonRpcError> {
    let call_value = match params {
        Value::Array(mut values) => values
            .drain(..)
            .next()
            .ok_or_else(|| invalid_params("call requires a call object"))?,
        value @ Value::Object(_) => value,
        _ => return Err(invalid_params("call params must be an array")),
    };
    serde_json::from_value(call_value).map_err(|error| invalid_params(&error.to_string()))
}

fn invalid_params(message: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: message.into(),
        data: None,
    }
}
