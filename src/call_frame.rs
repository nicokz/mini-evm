use crate::env::Address;
use ruint::aliases::U256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallContext {
    pub caller: Address,
    pub code_address: Address,
    pub storage_address: Address,
    pub value: U256,
    pub is_static: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame {
    pub gas_limit: u64,
    pub target: Address,
    pub is_static: bool,
    pub value: U256,
    pub input: Vec<u8>,
}
