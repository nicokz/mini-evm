use crate::env::Address;
use ruint::aliases::U256;

pub type I256 = i128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapHop {
    pub target_dex: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: Option<U256>,
    pub min_amount_out: U256,
    pub calldata: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbRoute {
    pub borrow_capital: Option<U256>,
    pub starting_token: Address,
    pub initial_amount: U256,
    pub hops: Vec<SwapHop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbSimulationResult {
    pub success: bool,
    pub starting_balance: U256,
    pub ending_balance: U256,
    pub net_profit_or_loss: I256,
    pub total_gas_used: u64,
    pub gas_cost_in_token: U256,
    pub failed_hop_index: Option<usize>,
    pub revert_reason: Option<String>,
}
