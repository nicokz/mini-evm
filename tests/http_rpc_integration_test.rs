use axum::Router;
use mini_evm::rpc::{RpcState, server::app};
use mini_evm::state::StateFork;
use reqwest::Client;
use serde_json::{Value, json};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let application: Router = app(RpcState::new(StateFork::default()));

    tokio::spawn(async move {
        axum::serve(listener, application).await.unwrap();
    });

    format!("http://{address}/")
}

async fn rpc_call(client: &Client, url: &str, id: u64, method: &str, params: Value) -> Value {
    client
        .post(url)
        .json(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn test_http_rpc_chain_and_block_handshake() {
    let url = spawn_test_server().await;
    let client = Client::new();

    let response = rpc_call(&client, &url, 1, "eth_chainId", json!([])).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"], "0x1");

    let response = rpc_call(&client, &url, 2, "net_version", json!([])).await;
    assert_eq!(response["result"], "1");

    let response = rpc_call(&client, &url, 3, "eth_blockNumber", json!([])).await;
    assert_eq!(response["result"], "0x0");

    let response = rpc_call(&client, &url, 4, "evm_mine", json!([])).await;
    assert!(response["result"].as_str().unwrap().starts_with("0x"));

    let response = rpc_call(&client, &url, 5, "eth_blockNumber", json!([])).await;
    assert_eq!(response["result"], "0x1");

    let response = rpc_call(
        &client,
        &url,
        6,
        "eth_getBlockByNumber",
        json!(["latest", false]),
    )
    .await;
    assert_eq!(response["id"], 6);
    assert!(
        response["result"]["number"]
            .as_str()
            .unwrap()
            .starts_with("0x")
    );
    assert!(response["result"]["transactions"].is_array());
}

#[tokio::test]
async fn test_http_rpc_eth_call_and_storage() {
    let url = spawn_test_server().await;
    let client = Client::new();
    let zero_address = "0x0000000000000000000000000000000000000000";

    let response = rpc_call(
        &client,
        &url,
        5,
        "eth_call",
        json!([{"to": zero_address, "data": "0x"}, "latest"]),
    )
    .await;
    assert_eq!(response["id"], 5);
    assert_eq!(response["result"], "0x");

    let response = rpc_call(
        &client,
        &url,
        6,
        "eth_getStorageAt",
        json!([zero_address, "0x0", "latest"]),
    )
    .await;
    assert_eq!(response["result"], format!("0x{}", "00".repeat(32)));

    let response = rpc_call(
        &client,
        &url,
        7,
        "eth_getCode",
        json!([zero_address, "latest"]),
    )
    .await;
    assert_eq!(response["result"], "0x");
}

#[tokio::test]
async fn test_http_rpc_receipt_and_unknown_method() {
    let url = spawn_test_server().await;
    let client = Client::new();

    let response = rpc_call(
        &client,
        &url,
        8,
        "eth_getTransactionReceipt",
        json!([format!("0x{}", "00".repeat(32))]),
    )
    .await;
    assert_eq!(response["result"], Value::Null);

    let response = rpc_call(&client, &url, 99, "non_existent_method", json!([])).await;
    assert_eq!(response["id"], 99);
    assert_eq!(response["error"]["code"], -32601);
}
