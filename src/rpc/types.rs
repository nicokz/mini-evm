use crate::env::Address;
use ruint::aliases::U256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::state::StateFork;
use crate::vm::runner::TxExecutionReceipt;
use crate::{mempool::TxPool, vm::runner::apply_signed_transaction};

#[derive(Clone)]
pub struct RpcState {
    pub state: Arc<RwLock<StateFork>>,
    pub receipts: Arc<RwLock<HashMap<[u8; 32], TxExecutionReceipt>>>,
    pub tx_pool: Arc<TxPool>,
    pub block_number: Arc<RwLock<u64>>,
    pub state_root: Arc<RwLock<[u8; 32]>>,
    pub auto_mine: Arc<AtomicBool>,
}

impl RpcState {
    pub fn new(state: StateFork) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
            receipts: Arc::new(RwLock::new(HashMap::new())),
            tx_pool: Arc::new(TxPool::new()),
            block_number: Arc::new(RwLock::new(0)),
            state_root: Arc::new(RwLock::new([0; 32])),
            auto_mine: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn set_auto_mine(&self, enabled: bool) {
        self.auto_mine.store(enabled, Ordering::Relaxed);
    }

    pub fn is_auto_mine(&self) -> bool {
        self.auto_mine.load(Ordering::Relaxed)
    }

    pub async fn mine_block(&self) -> Result<[u8; 32], String> {
        let senders = self.tx_pool.senders()?;
        let mut state = self.state.write().await;
        let mut current_nonces = HashMap::new();
        for sender in senders {
            current_nonces.insert(sender, state.get_account(&sender).nonce);
        }
        let pending = self.tx_pool.pop_executable(&current_nonces, 30_000_000)?;
        let mut receipts = self.receipts.write().await;
        for transaction in pending {
            let receipt = apply_signed_transaction(&mut state, &transaction.tx)
                .map_err(|error| format!("transaction execution failed: {error:?}"))?;
            receipts.insert(transaction.tx.hash.into(), receipt);
            state.clear_transient_storage();
        }

        let new_state_root = state.compute_state_root();
        *self.state_root.write().await = new_state_root;
        let mut block_number = self.block_number.write().await;
        *block_number = block_number.saturating_add(1);
        let mut block_hash = [0u8; 32];
        block_hash[..8].copy_from_slice(&block_number.to_be_bytes());
        Ok(block_hash)
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn custom(code: i32, message: impl Into<String>) -> Self {
        Self {
            code: code as i64,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(&self.0)))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        decode_hex(&value)
            .map(Bytes)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallRequest {
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub gas: Option<U256>,
    pub gas_price: Option<U256>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
}

impl<'de> Deserialize<'de> for CallRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let object = serde_json::Map::<String, Value>::deserialize(deserializer)?;
        Ok(Self {
            from: optional_hex(object.get("from"))
                .map(parse_address)
                .transpose()
                .map_err(serde::de::Error::custom)?,
            to: optional_hex(object.get("to"))
                .map(parse_address)
                .transpose()
                .map_err(serde::de::Error::custom)?,
            gas: optional_hex(object.get("gas"))
                .map(parse_u256)
                .transpose()
                .map_err(serde::de::Error::custom)?,
            gas_price: optional_hex(object.get("gasPrice"))
                .map(parse_u256)
                .transpose()
                .map_err(serde::de::Error::custom)?,
            value: optional_hex(object.get("value"))
                .map(parse_u256)
                .transpose()
                .map_err(serde::de::Error::custom)?,
            data: object
                .get("data")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(serde::de::Error::custom)?,
        })
    }
}

impl Serialize for CallRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serde_json::Map::new();
        if let Some(value) = self.from {
            object.insert(
                "from".into(),
                Value::String(format!("0x{}", hex::encode(value))),
            );
        }
        if let Some(value) = self.to {
            object.insert(
                "to".into(),
                Value::String(format!("0x{}", hex::encode(value))),
            );
        }
        if let Some(value) = self.gas {
            object.insert("gas".into(), Value::String(format_u256(value)));
        }
        if let Some(value) = self.gas_price {
            object.insert("gasPrice".into(), Value::String(format_u256(value)));
        }
        if let Some(value) = self.value {
            object.insert("value".into(), Value::String(format_u256(value)));
        }
        if let Some(value) = &self.data {
            object.insert(
                "data".into(),
                serde_json::to_value(value).map_err(serde::ser::Error::custom)?,
            );
        }
        object.serialize(serializer)
    }
}

fn optional_hex(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() % 2 == 1 {
        return hex::decode(format!("0{value}")).map_err(|error| error.to_string());
    }
    hex::decode(value).map_err(|error| error.to_string())
}

pub fn decode_address(value: &str) -> Result<Address, String> {
    parse_address(value.to_owned())
}

pub fn decode_u256(value: &str) -> Result<U256, String> {
    parse_u256(value.to_owned())
}

fn parse_address(value: String) -> Result<Address, String> {
    let bytes = decode_hex(&value)?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| format!("address must be 20 bytes, got {}", bytes.len()))
}

fn parse_u256(value: String) -> Result<U256, String> {
    let bytes = decode_hex(&value)?;
    if bytes.len() > 32 {
        return Err("integer exceeds 256 bits".into());
    }
    let mut padded = [0u8; 32];
    padded[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(U256::from_be_bytes(padded))
}

fn format_u256(value: U256) -> String {
    let bytes = value.to_be_bytes::<32>();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(31);
    format!("0x{}", hex::encode(&bytes[first..]))
}
