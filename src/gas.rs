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

/// Calculates the EIP-4844 blob base fee from excess blob gas.
pub fn calc_data_fee(excess_blob_gas: U256) -> U256 {
    const MIN_BLOB_BASE_FEE: u64 = 1;
    const BLOB_BASE_FEE_UPDATE_FRACTION: u64 = 3_338_477;

    let denominator = U256::from(BLOB_BASE_FEE_UPDATE_FRACTION);
    let mut numerator_accum = U256::from(MIN_BLOB_BASE_FEE) * denominator;
    let mut output = U256::ZERO;
    let mut i = U256::ONE;

    while numerator_accum > U256::ZERO {
        output += numerator_accum;
        numerator_accum = numerator_accum
            .checked_mul(excess_blob_gas)
            .and_then(|value| value.checked_div(denominator * i))
            .unwrap_or(U256::ZERO);
        i += U256::ONE;
    }

    output / denominator
}
