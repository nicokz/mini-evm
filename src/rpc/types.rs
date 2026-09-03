use crate::env::Address;
use ruint::aliases::U256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;


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
    hex::decode(value.strip_prefix("0x").unwrap_or(value)).map_err(|error| error.to_string())
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
