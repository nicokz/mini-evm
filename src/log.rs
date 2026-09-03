use crate::env::Address;
use ruint::aliases::U256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub address: Address,
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}
