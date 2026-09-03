use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use mini_evm::rpc::{RpcState, server::app};
use mini_evm::state::{AccountState, StateFork};
use ruint::aliases::U256;
use serde_json::json;
use tower::ServiceExt;

fn address(last_byte: u8) -> [u8; 20] {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

async fn post(state: StateFork, method: &str, params: serde_json::Value) -> serde_json::Value {
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
                "id": 1
            }))
            .unwrap(),
        ))
        .unwrap();
    let response = app(RpcState::new(state)).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_eth_get_code_deployed_contract() {
    let contract = address(1);
    let eoa = address(2);
    let mut state = StateFork::default();
    state.base_state.insert(
        contract,
        AccountState {
            code: vec![0x60, 0x42, 0x00],
            ..AccountState::default()
        },
    );

    let response = post(
        state.clone(),
        "eth_getCode",
        json!([format!("0x{}", hex::encode(contract)), "latest"]),
    )
    .await;
    assert_eq!(response["result"], "0x604200");

    let response = post(
        state,
        "eth_getCode",
        json!([format!("0x{}", hex::encode(eoa)), "latest"]),
    )
    .await;
    assert_eq!(response["result"], "0x");
}

#[tokio::test]
async fn test_eth_get_storage_at_dirty_and_base() {
    let contract = address(3);
    let mut state = StateFork::default();
    state.base_state.insert(
        contract,
        AccountState {
            storage: [(U256::from(1u8), U256::from(41u8))].into_iter().collect(),
            ..AccountState::default()
        },
    );
    state.set_storage(contract, U256::from(1u8), U256::from(42u8));

    let response = post(
        state.clone(),
        "eth_getStorageAt",
        json!([format!("0x{}", hex::encode(contract)), "0x01", "latest"]),
    )
    .await;
    assert_eq!(
        response["result"],
        "0x000000000000000000000000000000000000000000000000000000000000002a"
    );

    let response = post(
        state,
        "eth_getStorageAt",
        json!([format!("0x{}", hex::encode(contract)), "0x02", "latest"]),
    )
    .await;
    assert_eq!(
        response["result"],
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}
