use super::erc20::{get_erc20_balance, set_erc20_balance_override};
use super::types::{ArbRoute, ArbSimulationResult, I256};
use crate::env::Address;
use crate::simulation::simulate_tx;
use crate::state::StateFork;
use ruint::aliases::U256;

pub struct ArbSimulator<'a> {
    pub base_state: &'a StateFork,
    pub sender: Address,
}

impl<'a> ArbSimulator<'a> {
    pub fn new(base_state: &'a StateFork, sender: Address) -> Self {
        Self { base_state, sender }
    }

    pub fn simulate_route(&self, route: &ArbRoute) -> ArbSimulationResult {
        let mut fork_state = self.base_state.clone();
        let starting_balance = get_erc20_balance(&fork_state, route.starting_token, self.sender);
        let mut current_amount = route.initial_amount;
        let mut total_gas_used = 0u64;

        for (index, hop) in route.hops.iter().enumerate() {
            let amount_in = hop.amount_in.unwrap_or(current_amount);
            let result = simulate_tx(
                &fork_state,
                self.sender,
                hop.target_dex,
                U256::ZERO,
                &hop.calldata,
                30_000_000,
            );
            total_gas_used = total_gas_used.saturating_add(result.gas_used);
            if !result.success {
                return self.failure(
                    starting_balance,
                    total_gas_used,
                    index,
                    if result.return_data.is_empty() {
                        "hop execution failed".into()
                    } else {
                        format!("0x{}", hex::encode(result.return_data))
                    },
                );
            }
            for (address, account) in result.state_diff {
                fork_state.dirty_state.insert(address, account);
            }
            current_amount = decode_amount(&result.return_data).unwrap_or(amount_in);
            set_erc20_balance_override(&mut fork_state, hop.token_out, self.sender, current_amount);
            if current_amount < hop.min_amount_out {
                return self.failure(
                    starting_balance,
                    total_gas_used,
                    index,
                    "minimum output amount not met".into(),
                );
            }
        }

        let ending_balance = get_erc20_balance(&fork_state, route.starting_token, self.sender);
        self.success(starting_balance, ending_balance, total_gas_used)
    }

    pub fn simulate_with_flashloan(
        &self,
        _lender: Address,
        route: &ArbRoute,
    ) -> ArbSimulationResult {
        self.simulate_route(route)
    }

    pub fn find_optimal_input_amount(
        &self,
        route: &mut ArbRoute,
        min_in: U256,
        max_in: U256,
        steps: usize,
    ) -> (U256, I256) {
        let count = steps.max(1);
        let mut best_amount = min_in;
        let mut best_profit = i128::MIN;
        for step in 0..=count {
            let amount = min_in + (max_in - min_in) * U256::from(step) / U256::from(count);
            route.initial_amount = amount;
            let result = self.simulate_route(route);
            if result.success && result.net_profit_or_loss > best_profit {
                best_amount = amount;
                best_profit = result.net_profit_or_loss;
            }
        }
        (best_amount, best_profit)
    }

    fn success(
        &self,
        starting_balance: U256,
        ending_balance: U256,
        gas: u64,
    ) -> ArbSimulationResult {
        ArbSimulationResult {
            success: true,
            starting_balance,
            ending_balance,
            net_profit_or_loss: signed_difference(ending_balance, starting_balance),
            total_gas_used: gas,
            gas_cost_in_token: U256::ZERO,
            failed_hop_index: None,
            revert_reason: None,
        }
    }

    fn failure(
        &self,
        starting_balance: U256,
        gas: u64,
        index: usize,
        reason: String,
    ) -> ArbSimulationResult {
        ArbSimulationResult {
            success: false,
            starting_balance,
            ending_balance: starting_balance,
            net_profit_or_loss: 0,
            total_gas_used: gas,
            gas_cost_in_token: U256::ZERO,
            failed_hop_index: Some(index),
            revert_reason: Some(reason),
        }
    }
}

fn decode_amount(data: &[u8]) -> Option<U256> {
    if data.len() < 32 {
        return None;
    }
    let mut word = [0u8; 32];
    word.copy_from_slice(&data[data.len() - 32..]);
    Some(U256::from_be_bytes(word))
}

fn signed_difference(left: U256, right: U256) -> I256 {
    let left = i128::try_from(left).unwrap_or(i128::MAX);
    let right = i128::try_from(right).unwrap_or(i128::MAX);
    left.saturating_sub(right)
}
