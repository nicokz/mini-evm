use super::RpcState;
use super::handler::{
    handle_eth_block_number, handle_eth_call, handle_eth_estimate_gas,
    handle_eth_get_block_by_number, handle_eth_get_code, handle_eth_get_storage_at,
    handle_eth_get_transaction_receipt, handle_eth_send_raw_transaction, handle_evm_mine,
    handle_pending_transactions, handle_txpool_status,
};
use super::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::Serialize;
use serde_json::Value;
use tower_http::trace::TraceLayer;

pub fn app(rpc_state: RpcState) -> Router {
    Router::new()
        .route("/", post(rpc_endpoint))
        .layer(TraceLayer::new_for_http())
        .with_state(rpc_state)
}

async fn rpc_endpoint(
    State(state): State<RpcState>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let response = match request.method.as_str() {
        "eth_call" => match handle_eth_call(&state, request.params).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_estimateGas" => match handle_eth_estimate_gas(&state, request.params).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_getStorageAt" => match handle_eth_get_storage_at(&state, request.params).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_getCode" => match handle_eth_get_code(&state, request.params).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_sendRawTransaction" => {
            match handle_eth_send_raw_transaction(&state, request.params).await {
                Ok(result) => success(result, request.id),
                Err(error) => error_response(error, request.id),
            }
        }
        "eth_blockNumber" => success(handle_eth_block_number(&state).await, request.id),
        "evm_mine" | "miner_mine" => match handle_evm_mine(&state).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "txpool_status" => match handle_txpool_status(&state).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_pendingTransactions" => match handle_pending_transactions(&state).await {
            Ok(result) => success(result, request.id),
            Err(error) => error_response(error, request.id),
        },
        "eth_getBlockByNumber" => {
            match handle_eth_get_block_by_number(&state, &request.params).await {
                Ok(result) => success(result, request.id),
                Err(error) => error_response(error, request.id),
            }
        }
        "eth_getTransactionReceipt" => {
            match handle_eth_get_transaction_receipt(&state, &request.params).await {
                Ok(result) => success(result, request.id),
                Err(error) => error_response(error, request.id),
            }
        }
        "net_version" => success("1".to_owned(), request.id),
        "eth_chainId" => success("0x1".to_owned(), request.id),
        _ => error_response(
            JsonRpcError {
                code: -32601,
                message: "Method not found".into(),
                data: None,
            },
            request.id,
        ),
    };
    (StatusCode::OK, Json(response))
}

fn success<T: Serialize>(result: T, id: Value) -> JsonRpcResponse<Value> {
    JsonRpcResponse {
        jsonrpc: "2.0",
        result: Some(serde_json::to_value(result).expect("RPC result must serialize")),
        error: None,
        id,
    }
}

fn error_response(error: JsonRpcError, id: Value) -> JsonRpcResponse<Value> {
    JsonRpcResponse {
        jsonrpc: "2.0",
        result: None,
        error: Some(error),
        id,
    }
}
