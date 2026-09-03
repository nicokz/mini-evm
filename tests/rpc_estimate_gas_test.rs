use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use mini_evm::opcodes::*;
use mini_evm::rpc::{RpcState, server::app, types::CallRequest};
use mini_evm::state::{AccountState, StateFork};
use ruint::aliases::U256;
use serde_json::json;
use tower::ServiceExt;

fn address(last_byte: u8) -> [u8; 20] {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

async fn post_call(state: StateFork, call: CallRequest, id: u64) -> serde_json::Value {
    let service = app(RpcState::new(state));
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "method": "eth_estimateGas",
                "params": [call, "latest"],
                "id": id
            }))
            .unwrap(),
        ))
        .unwrap();
    let response = service.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_estimate_gas_simple_transfer() {
    let response = post_call(
        StateFork::default(),
        CallRequest {
            from: Some(address(1)),
            to: Some(address(2)),
            ..CallRequest::default()
        },
        1,
    )
    .await;

    assert_eq!(response["result"], "0x5208");
}

#[tokio::test]
async fn test_estimate_gas_memory_contract_exceeds_intrinsic() {
    let target = address(2);
    let mut state = StateFork::default();
    state.base_state.insert(
        target,
        AccountState {
            code: vec![PUSH1, 0xff, PUSH2, 0x04, 0x00, MSTORE8],
            ..AccountState::default()
        },
    );

    let response = post_call(
        state,
        CallRequest {
            to: Some(target),
            ..CallRequest::default()
        },
        2,
    )
    .await;

    let estimated = u64::from_str_radix(
        response["result"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    assert!(estimated > 21_000);
    assert!(estimated < 30_000_000);
}

#[tokio::test]
async fn test_estimate_gas_reverting_call_fails() {
    let target = address(3);
    let mut state = StateFork::default();
    state.base_state.insert(
        target,
        AccountState {
            code: vec![PUSH1, 0, PUSH1, 0, REVERT],
            ..AccountState::default()
        },
    );

    let response = post_call(
        state,
        CallRequest {
            to: Some(target),
            value: Some(U256::ZERO),
            ..CallRequest::default()
        },
        3,
    )
    .await;

    assert_eq!(response["error"]["code"], -32000);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("reverted")
    );
}
