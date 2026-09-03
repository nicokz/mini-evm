use ruint::aliases::U256;
use std::collections::HashMap;

pub type Address = [u8; 20];
pub type BlockHashFn = fn(u64) -> U256;
pub type BlockContext = Environment;

fn default_block_hash(_number: u64) -> U256 {
    U256::ZERO
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub chain_id: U256,
    pub origin: Address,
    pub gas_price: U256,
    pub coinbase: Address,
    pub timestamp: U256,
    pub number: U256,
    pub prevrandao: U256,
    pub gas_limit: U256,
    pub base_fee: U256,
    pub blob_base_fee: U256,
    pub block_hash: BlockHashFn,
    pub block_hashes: HashMap<u64, U256>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            chain_id: U256::from(1u8),
            origin: [0; 20],
            gas_price: U256::ZERO,
            coinbase: [0; 20],
            timestamp: U256::ZERO,
            number: U256::ZERO,
            prevrandao: U256::ZERO,
            gas_limit: U256::from(30_000_000u64),
            base_fee: U256::ZERO,
            blob_base_fee: U256::ZERO,
            block_hash: default_block_hash,
            block_hashes: HashMap::new(),
        }
    }
}
