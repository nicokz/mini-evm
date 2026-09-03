use mini_evm::env::Address;
use ruint::aliases::U256;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[allow(dead_code)]
pub type TestFixture = HashMap<String, StateTestCase>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateTestCase {
    pub env: Env,
    #[serde(deserialize_with = "deserialize_address_map")]
    pub pre: HashMap<Address, AccountFixture>,
    pub transaction: TxFixture,
    pub post: HashMap<String, Vec<PostStateExpectation>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Env {
    #[serde(deserialize_with = "deserialize_address")]
    pub current_coinbase: Address,
    #[allow(dead_code)]
    #[serde(deserialize_with = "deserialize_u256")]
    pub current_difficulty: U256,
    #[serde(deserialize_with = "deserialize_u256")]
    pub current_gas_limit: U256,
    #[serde(deserialize_with = "deserialize_u256")]
    pub current_number: U256,
    #[serde(deserialize_with = "deserialize_u256")]
    pub current_timestamp: U256,
    #[serde(default, deserialize_with = "deserialize_u256_opt")]
    pub current_base_fee: Option<U256>,
    #[serde(default, deserialize_with = "deserialize_u256_opt")]
    pub current_random: Option<U256>,
    #[serde(default, deserialize_with = "deserialize_u256_opt")]
    pub current_excess_blob_gas: Option<U256>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountFixture {
    #[serde(deserialize_with = "deserialize_u256")]
    pub balance: U256,
    #[serde(deserialize_with = "deserialize_bytes")]
    pub code: Vec<u8>,
    #[serde(deserialize_with = "deserialize_u64")]
    pub nonce: u64,
    #[serde(default, deserialize_with = "deserialize_storage")]
    pub storage: HashMap<U256, U256>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxFixture {
    #[serde(deserialize_with = "deserialize_bytes_vec")]
    pub data: Vec<Vec<u8>>,
    #[serde(deserialize_with = "deserialize_u256_vec")]
    pub gas_limit: Vec<U256>,
    #[serde(deserialize_with = "deserialize_u256_vec")]
    pub gas_price: Vec<U256>,
    #[serde(deserialize_with = "deserialize_u256_vec")]
    pub value: Vec<U256>,
    #[allow(dead_code)]
    #[serde(deserialize_with = "deserialize_u256")]
    pub nonce: U256,
    #[serde(default, deserialize_with = "deserialize_address_opt")]
    pub to: Option<Address>,
    #[serde(default, deserialize_with = "deserialize_address_opt")]
    pub sender: Option<Address>,
    pub secret_key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_bytes_vec_opt")]
    pub blob_versioned_hashes: Option<Vec<Vec<u8>>>,
    #[serde(default, deserialize_with = "deserialize_u256_vec_opt")]
    pub max_fee_per_blob_gas: Option<Vec<U256>>,
    #[allow(dead_code)]
    #[serde(default, deserialize_with = "deserialize_u256_vec_opt")]
    pub max_fee_per_gas: Option<Vec<U256>>,
    #[allow(dead_code)]
    #[serde(default, deserialize_with = "deserialize_u256_vec_opt")]
    pub max_priority_fee_per_gas: Option<Vec<U256>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PostStateExpectation {
    pub hash: String,
    #[serde(default)]
    pub indexes: Value,
    #[serde(default)]
    pub logs: Value,
}

fn string(value: &Value) -> Result<&str, String> {
    value.as_str().ok_or_else(|| "expected hex string".into())
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    hex::decode(value.strip_prefix("0x").unwrap_or(value)).map_err(|error| error.to_string())
}

fn parse_u256(value: &Value) -> Result<U256, String> {
    let bytes = hex_bytes(string(value)?)?;
    if bytes.len() > 32 {
        return Err("integer exceeds 256 bits".into());
    }
    let mut padded = [0u8; 32];
    padded[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(U256::from_be_bytes(padded))
}

fn parse_address(value: &Value) -> Result<Address, String> {
    let bytes = hex_bytes(string(value)?)?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| format!("address must be 20 bytes, got {}", bytes.len()))
}

fn deserialize_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: serde::Deserializer<'de>,
{
    parse_u256(&Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

fn deserialize_u256_opt<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer)?
        .map(|value| parse_u256(&value))
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn deserialize_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    parse_u256(&Value::deserialize(deserializer)?)
        .and_then(|value| value.try_into().map_err(|_| "integer exceeds u64".into()))
        .map_err(serde::de::Error::custom)
}

fn deserialize_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    parse_address(&Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

fn deserialize_address_opt<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer)?
        .map(|value| parse_address(&value))
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn deserialize_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    hex_bytes(string(&Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)?)
        .map_err(serde::de::Error::custom)
}

fn deserialize_bytes_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<Value>::deserialize(deserializer)?
        .iter()
        .map(|value| hex_bytes(string(value)?))
        .collect::<Result<_, _>>()
        .map_err(serde::de::Error::custom)
}

fn deserialize_u256_vec<'de, D>(deserializer: D) -> Result<Vec<U256>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<Value>::deserialize(deserializer)?
        .iter()
        .map(parse_u256)
        .collect::<Result<_, _>>()
        .map_err(serde::de::Error::custom)
}

fn deserialize_u256_vec_opt<'de, D>(deserializer: D) -> Result<Option<Vec<U256>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<Value>>::deserialize(deserializer)?
        .map(|values| values.iter().map(parse_u256).collect::<Result<_, _>>())
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn deserialize_bytes_vec_opt<'de, D>(deserializer: D) -> Result<Option<Vec<Vec<u8>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<Value>>::deserialize(deserializer)?
        .map(|values| {
            values
                .iter()
                .map(|value| hex_bytes(string(value)?))
                .collect::<Result<_, _>>()
        })
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn deserialize_storage<'de, D>(deserializer: D) -> Result<HashMap<U256, U256>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let object = HashMap::<String, Value>::deserialize(deserializer)?;
    object
        .into_iter()
        .map(|(key, value)| {
            let key = parse_u256(&Value::String(key)).map_err(serde::de::Error::custom)?;
            let value = parse_u256(&value).map_err(serde::de::Error::custom)?;
            Ok((key, value))
        })
        .collect()
}

fn deserialize_address_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<Address, AccountFixture>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let object = HashMap::<String, AccountFixture>::deserialize(deserializer)?;
    object
        .into_iter()
        .map(|(key, value)| {
            let address = parse_address(&Value::String(key)).map_err(serde::de::Error::custom)?;
            Ok((address, value))
        })
        .collect()
}
