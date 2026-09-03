use crate::crypto::keccak256;
use crate::env::Address;
use crate::simulation::simulate_tx;
use crate::state::StateFork;
use ruint::aliases::U256;

pub fn get_erc20_balance(state: &StateFork, token: Address, owner: Address) -> U256 {
    let mut calldata = vec![0x70, 0xa0, 0x82, 0x31];
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(&owner);
    let result = simulate_tx(state, owner, token, U256::ZERO, &calldata, 100_000);
    if result.success && result.return_data.len() >= 32 {
        let mut word = [0u8; 32];
        word.copy_from_slice(&result.return_data[result.return_data.len() - 32..]);
        U256::from_be_bytes(word)
    } else {
        let mut key = [0u8; 64];
        key[12..32].copy_from_slice(&owner);
        let slot = keccak256(&key);
        state.get_storage(&token, U256::from_be_bytes(slot))
    }
}

pub fn set_erc20_balance_override(
    state: &mut StateFork,
    token: Address,
    owner: Address,
    amount: U256,
) {
    let mut key = [0u8; 64];
    key[12..32].copy_from_slice(&owner);
    let slot = keccak256(&key);
    state.set_storage(token, U256::from_be_bytes(slot), amount);
}
