use futures_util::{SinkExt, StreamExt};
use mini_evm::rpc::RpcState;
use mini_evm::rpc::server::app;
use mini_evm::rpc::types::{ActiveSubscriptions, LogFilter, LogNotification, SubscriptionKind};
use mini_evm::state::StateFork;
use std::future::IntoFuture;
use tokio_tungstenite::tungstenite::Message;

#[test]
fn log_filter_matches_address_and_topics() {
    let log = LogNotification {
        address: "0x0000000000000000000000000000000000000001".into(),
        topics: vec!["0x01".into(), "0x02".into()],
        data: "0x".into(),
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        log_index: None,
    };
    let filter = LogFilter {
        address: Some(vec!["0X0000000000000000000000000000000000000001".into()]),
        topics: Some(vec![Some(vec!["0x01".into()]), None]),
    };

    assert!(filter.matches(&log));
}

#[test]
fn subscriptions_generate_ids_and_remove_only_existing_entries() {
    let mut subscriptions = ActiveSubscriptions::new();
    let id = subscriptions.add(SubscriptionKind::NewHeads);

    assert!(id.starts_with("0x"));
    assert!(subscriptions.remove(&id));
    assert!(!subscriptions.remove(&id));
}

#[tokio::test]
async fn websocket_subscribes_notifies_and_unsubscribes() {
    let state = RpcState::new(StateFork::default());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, app(state.clone())).into_future());
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
        .await
        .unwrap();

    socket
        .send(Message::Text(
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_subscribe","params":["newHeads"]}"#.into(),
        ))
        .await
        .unwrap();
    let subscription = socket.next().await.unwrap().unwrap();
    let subscription: serde_json::Value =
        serde_json::from_str(subscription.into_text().unwrap().as_ref()).unwrap();
    let subscription_id = subscription["result"].as_str().unwrap().to_owned();

    state
        .events
        .send(mini_evm::rpc::types::EvmEvent::NewHead(
            mini_evm::rpc::types::BlockHeaderNotification {
                number: "0x1".into(),
                hash: "0x01".into(),
                parent_hash: "0x00".into(),
                miner: "0x00".into(),
                timestamp: "0x0".into(),
            },
        ))
        .unwrap();
    let notification = socket.next().await.unwrap().unwrap();
    let notification: serde_json::Value =
        serde_json::from_str(notification.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(notification["params"]["subscription"], subscription_id);
    assert_eq!(notification["params"]["result"]["number"], "0x1");

    socket
        .send(Message::Text(
            format!(r#"{{"jsonrpc":"2.0","id":2,"method":"eth_unsubscribe","params":["{subscription_id}"]}}"#),
        ))
        .await
        .unwrap();
    let response = socket.next().await.unwrap().unwrap();
    let response: serde_json::Value =
        serde_json::from_str(response.into_text().unwrap().as_ref()).unwrap();
    assert_eq!(response["result"], true);

    server.abort();
}
