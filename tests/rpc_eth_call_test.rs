use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use mini_evm::opcodes::*;
use mini_evm::rpc::{RpcState, server::app, types::CallRequest};
use mini_evm::state::{AccountState, StateFork};
use serde_json::json;
use tower::ServiceExt;

fn address(last_byte: u8) -> [u8; 20] {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

#[tokio::test]
async fn eth_call_returns_contract_output() {
    let target = address(10);
    let code = vec![PUSH1, 0x42, PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN];
    let mut state = StateFork::default();
    state.base_state.insert(
        target,
        AccountState {
            code,
            ..AccountState::default()
        },
    );
    let service = app(RpcState::new(state));
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "method": "eth_call",
                "params": [CallRequest {
                    to: Some(target),
                    ..CallRequest::default()
                }, "latest"],
                "id": 1
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = service.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        response,
        json!({
            "jsonrpc": "2.0",
            "result": "0x0000000000000000000000000000000000000000000000000000000000000042",
            "id": 1
        })
    );
}

#[tokio::test]
async fn rpc_dispatches_chain_id_and_unknown_methods() {
    let service = app(RpcState::new(StateFork::default()));
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}"#,
        ))
        .unwrap();
    let response = service.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["result"],
        "0x1"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","method":"no_such_method","params":[],"id":2}"#,
        ))
        .unwrap();
    let response = service.oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"]["code"],
        -32601
    );
}
