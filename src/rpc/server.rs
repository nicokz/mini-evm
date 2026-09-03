use super::RpcState;
use super::handler::{
    handle_eth_block_number, handle_eth_call, handle_eth_estimate_gas,
    handle_eth_get_block_by_number, handle_eth_get_code, handle_eth_get_storage_at,
    handle_eth_get_transaction_receipt, handle_eth_send_raw_transaction, handle_evm_mine,
    handle_pending_transactions, handle_txpool_status,
};
use super::types::{ActiveSubscriptions, EvmEvent, SubscriptionKind};
use super::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::fuzzer::{FuzzReport, FuzzRequest};
use axum::{
    Json, Router,
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tower_http::trace::TraceLayer;

pub fn app(rpc_state: RpcState) -> Router {
    Router::new()
        .route("/", post(rpc_endpoint))
        .route("/ws", get(ws_upgrade))
        .route("/v1/fuzz", post(fuzz_endpoint))
        .layer(TraceLayer::new_for_http())
        .with_state(rpc_state)
}

async fn fuzz_endpoint(
    State(state): State<RpcState>,
    Json(request): Json<FuzzRequest>,
) -> impl IntoResponse {
    let base_state = state.state.read().await.clone();
    match tokio::task::spawn_blocking(move || request.run(&base_state)).await {
        Ok(Ok(report)) => (StatusCode::OK, Json(report)),
        Ok(Err(error)) => (StatusCode::BAD_REQUEST, Json(FuzzReport::error(error))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(FuzzReport::error(error.to_string())),
        ),
    }
}

async fn ws_upgrade(
    State(state): State<RpcState>,
    websocket: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| handle_ws(socket, state.events.subscribe()))
}

async fn handle_ws(socket: WebSocket, mut event_rx: tokio::sync::broadcast::Receiver<EvmEvent>) {
    let (mut sender, mut receiver) = socket.split();
    let mut subscriptions = ActiveSubscriptions::new();

    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(Ok(Message::Text(text))) = message else { break };
                let Ok(request) = serde_json::from_str::<Value>(&text) else { continue };
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
                let params = request.get("params").and_then(Value::as_array);
                let response = match method {
                    "eth_subscribe" => {
                        let kind = params.and_then(|values| values.first()).and_then(Value::as_str).and_then(|value| match value {
                            "newHeads" => Some(SubscriptionKind::NewHeads),
                            "pendingTransactions" => Some(SubscriptionKind::PendingTransactions),
                            "logs" => Some(SubscriptionKind::Logs(params.and_then(|values| values.get(1)).and_then(|value| serde_json::from_value(value.clone()).ok()).unwrap_or_default())),
                            _ => None,
                        });
                        match kind {
                            Some(kind) => success(subscriptions.add(kind), id),
                            None => error_response(JsonRpcError::custom(-32601, "Unsupported subscription type"), id),
                        }
                    }
                    "eth_unsubscribe" => {
                        let subscription = params.and_then(|values| values.first()).and_then(Value::as_str).unwrap_or_default();
                        success(subscriptions.remove(subscription), id)
                    }
                    _ => error_response(JsonRpcError::custom(-32601, "Method not found"), id),
                };
                if let Ok(payload) = serde_json::to_string(&response)
                    && sender.send(Message::Text(payload)).await.is_err()
                {
                    break;
                }
            }
            event = event_rx.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                for (subscription, kind) in &subscriptions.subs {
                    let result = match (kind, &event) {
                        (SubscriptionKind::NewHeads, EvmEvent::NewHead(header)) => Some(serde_json::to_value(header).expect("header serializes")),
                        (SubscriptionKind::PendingTransactions, EvmEvent::PendingTx(hash)) => Some(Value::String(hash.clone())),
                        (SubscriptionKind::Logs(filter), EvmEvent::Log(log)) if filter.matches(log) => Some(serde_json::to_value(log).expect("log serializes")),
                        _ => None,
                    };
                    if let Some(result) = result {
                        let notification = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "eth_subscription",
                            "params": { "subscription": subscription, "result": result }
                        });
                        if sender.send(Message::Text(notification.to_string())).await.is_err() { return; }
                    }
                }
            }
        }
    }
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
