use mini_evm::opcodes::*;
use mini_evm::sim::engine::ArbSimulator;
use mini_evm::sim::erc20::set_erc20_balance_override;
use mini_evm::sim::types::{ArbRoute, SwapHop};
use mini_evm::state::{AccountState, StateFork};
use ruint::aliases::U256;

fn address(last_byte: u8) -> [u8; 20] {
    let mut value = [0u8; 20];
    value[19] = last_byte;
    value
}

fn return_amount(amount: u8) -> Vec<u8> {
    vec![PUSH1, amount, PUSH1, 0, MSTORE, PUSH1, 32, PUSH1, 0, RETURN]
}

fn route(starting_token: [u8; 20], hops: Vec<SwapHop>) -> ArbRoute {
    ArbRoute {
        borrow_capital: None,
        starting_token,
        initial_amount: U256::from(100u8),
        hops,
    }
}

#[test]
fn test_2hop_profitable_arbitrage() {
    let sender = address(1);
    let token_x = address(2);
    let token_y = address(3);
    let dex_a = address(4);
    let dex_b = address(5);
    let mut state = StateFork::default();
    for (dex, amount) in [(dex_a, 200), (dex_b, 200)] {
        state.base_state.insert(
            dex,
            AccountState {
                code: return_amount(amount),
                ..AccountState::default()
            },
        );
    }
    set_erc20_balance_override(&mut state, token_x, sender, U256::from(100u8));
    let route = route(
        token_x,
        vec![
            SwapHop {
                target_dex: dex_a,
                token_in: token_x,
                token_out: token_y,
                amount_in: None,
                min_amount_out: U256::from(200u8),
                calldata: Vec::new(),
            },
            SwapHop {
                target_dex: dex_b,
                token_in: token_y,
                token_out: token_x,
                amount_in: None,
                min_amount_out: U256::from(200u8),
                calldata: Vec::new(),
            },
        ],
    );

    let result = ArbSimulator::new(&state, sender).simulate_route(&route);
    assert!(result.success);
    assert_eq!(result.starting_balance, U256::from(100u8));
    assert_eq!(result.ending_balance, U256::from(200u8));
    assert_eq!(result.net_profit_or_loss, 100);
}

#[test]
fn test_reverting_middle_hop_rolls_back_simulation() {
    let sender = address(1);
    let token_x = address(2);
    let token_y = address(3);
    let dex_a = address(4);
    let dex_c = address(5);
    let mut state = StateFork::default();
    state.base_state.insert(
        dex_a,
        AccountState {
            code: return_amount(200),
            ..AccountState::default()
        },
    );
    state.base_state.insert(
        dex_c,
        AccountState {
            code: vec![PUSH1, 0, PUSH1, 0, REVERT],
            ..AccountState::default()
        },
    );
    set_erc20_balance_override(&mut state, token_x, sender, U256::from(100u8));
    let route = route(
        token_x,
        vec![
            SwapHop {
                target_dex: dex_a,
                token_in: token_x,
                token_out: token_y,
                amount_in: None,
                min_amount_out: U256::from(200u8),
                calldata: Vec::new(),
            },
            SwapHop {
                target_dex: dex_c,
                token_in: token_y,
                token_out: token_x,
                amount_in: None,
                min_amount_out: U256::ZERO,
                calldata: Vec::new(),
            },
        ],
    );

    let result = ArbSimulator::new(&state, sender).simulate_route(&route);
    assert!(!result.success);
    assert_eq!(result.failed_hop_index, Some(1));
    assert_eq!(state.get_storage(&token_y, U256::ZERO), U256::ZERO);
}
