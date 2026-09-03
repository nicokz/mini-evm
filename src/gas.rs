use ruint::aliases::U256;

pub fn calculate_intrinsic_gas(input: &[u8], is_contract_creation: bool) -> u64 {
    let base = if is_contract_creation { 53_000 } else { 21_000 };
    input.iter().fold(base, |gas, byte| {
        gas.saturating_add(if *byte == 0 { 4 } else { 16 })
    })
}

#[inline]
pub fn calc_eip150_call_gas(available_gas: u64, requested_gas: U256) -> u64 {
    let max_call_gas = available_gas - (available_gas / 64);
    let requested = u64::try_from(requested_gas).unwrap_or(u64::MAX);
    requested.min(max_call_gas)
}
