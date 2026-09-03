use super::RpcState;
use super::types::{CallRequest, JsonRpcError, decode_address, decode_u256};
use crate::simulation::simulate_tx;
use crate::tx::decoder::decode_raw_tx;
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
        .ok_or_else(|| {
            JsonRpcError::invalid_params("eth_sendRawTransaction requires a raw transaction")
        })?;

    let bytes = hex::decode(raw.strip_prefix("0x").unwrap_or(raw))
        .map_err(|_| JsonRpcError::invalid_params("Invalid raw transaction hex"))?;

    let transaction = decode_raw_tx(&bytes).map_err(|e| {
        JsonRpcError::custom(
            -32000,
            format!("Invalid signature or transaction encoding: {e:?}"),
        )
    })?;

    let sender = *transaction.sender.as_fixed_bytes();
    let current_nonce = state.state.read().await.get_account(&sender).nonce;
    if transaction.nonce < current_nonce {
        return Err(JsonRpcError::custom(
            -32000,
            "nonce is lower than account nonce",
        ));
    }
    state
        .tx_pool
        .insert(sender, transaction.clone(), bytes)
        .map_err(|error| JsonRpcError::custom(-32000, error))?;
    if state.is_auto_mine() {
        state
            .mine_block()
            .await
            .map_err(|error| JsonRpcError::custom(-32000, error))?;
    }

    Ok(format!("0x{}", hex::encode(transaction.hash)))
}

pub async fn handle_txpool_status(state: &RpcState) -> Result<Value, JsonRpcError> {
    let pending = state
        .tx_pool
        .len()
        .map_err(|error| JsonRpcError::custom(-32000, error))?;
    Ok(serde_json::json!({
        "pending": format!("0x{pending:x}"),
        "queued": "0x0"
    }))
}

pub async fn handle_pending_transactions(state: &RpcState) -> Result<Value, JsonRpcError> {
    let transactions = state
        .tx_pool
        .pending()
        .map_err(|error| JsonRpcError::custom(-32000, error))?;
    Ok(Value::Array(
        transactions
            .into_iter()
            .map(|transaction| Value::String(format!("0x{}", hex::encode(transaction.tx.hash))))
            .collect(),
    ))
}

pub async fn handle_eth_block_number(state: &RpcState) -> String {
    format!("0x{:x}", *state.block_number.read().await)
}

pub async fn handle_evm_mine(state: &RpcState) -> Result<String, JsonRpcError> {
    let hash = state
        .mine_block()
        .await
        .map_err(|error| JsonRpcError::custom(-32000, error))?;
    Ok(format!("0x{}", hex::encode(hash)))
}

pub async fn handle_eth_get_block_by_number(
    state: &RpcState,
    params: &Value,
) -> Result<Value, JsonRpcError> {
    let values = params
        .as_array()
        .ok_or_else(|| invalid_params("eth_getBlockByNumber params must be an array"))?;
    if values.first().and_then(Value::as_str).is_none() {
        return Err(invalid_params("eth_getBlockByNumber requires a block tag"));
    }

    let block_number = *state.block_number.read().await;
    let state_root = *state.state_root.read().await;
    Ok(serde_json::json!({
        "number": format!("0x{block_number:x}"),
        "hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
        "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "nonce": "0x0000000000000000",
        "sha3Uncles": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "logsBloom": format!("0x{}", "00".repeat(256)),
        "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "stateRoot": format!("0x{}", hex::encode(state_root)),
        "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "miner": "0x0000000000000000000000000000000000000000",
        "difficulty": "0x0",
        "totalDifficulty": "0x0",
        "extraData": "0x",
        "size": "0x0",
        "gasLimit": "0x1c9c380",
        "gasUsed": "0x0",
        "timestamp": "0x60000000",
        "transactions": [],
        "uncles": []
    }))
}

pub async fn handle_eth_get_transaction_receipt(
    state: &RpcState,
    params: &Value,
) -> Result<Option<Value>, JsonRpcError> {
    let values = params
        .as_array()
        .ok_or_else(|| invalid_params("eth_getTransactionReceipt params must be an array"))?;
    let hash = values
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("eth_getTransactionReceipt requires a transaction hash"))?;
    let bytes = hex::decode(hash.strip_prefix("0x").unwrap_or(hash))
        .map_err(|_| invalid_params("invalid transaction hash"))?;
    let hash: [u8; 32] = bytes
        .try_into()
        .map_err(|_| invalid_params("transaction hash must be 32 bytes"))?;
    let receipt = state.receipts.read().await.get(&hash).cloned();

    Ok(receipt.map(|receipt| {
        serde_json::json!({
            "transactionHash": format!("0x{}", hex::encode(hash)),
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "blockNumber": "0x1",
            "from": "0x0000000000000000000000000000000000000000",
            "to": Value::Null,
            "cumulativeGasUsed": format!("0x{:x}", receipt.gas_used),
            "gasUsed": format!("0x{:x}", receipt.gas_used),
            "contractAddress": Value::Null,
            "logs": [],
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "status": format!("0x{:x}", receipt.status),
            "effectiveGasPrice": "0x1",
            "type": "0x0"
        })
    }))
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
