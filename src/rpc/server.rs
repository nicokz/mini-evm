use super::RpcState;
use super::handler::{
    handle_eth_call, handle_eth_estimate_gas, handle_eth_get_code, handle_eth_get_storage_at,
    handle_eth_send_raw_transaction,
};
use super::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
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

fn success<T>(result: T, id: serde_json::Value) -> JsonRpcResponse<T> {
    JsonRpcResponse {
        jsonrpc: "2.0",
        result: Some(result),
        error: None,
        id,
    }
}

fn error_response(error: JsonRpcError, id: serde_json::Value) -> JsonRpcResponse<String> {
    JsonRpcResponse {
        jsonrpc: "2.0",
        result: None,
        error: Some(error),
        id,
    }
}
