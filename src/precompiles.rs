use crate::env::Address;
use crate::vm::VmError;
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use ripemd::{Digest as RipemdDigest, Ripemd160};
use sha2::Sha256;

pub fn is_precompile(addr: &Address) -> bool {
    addr[..19].iter().all(|byte| *byte == 0) && (1..=4).contains(&addr[19])
}

pub fn execute_precompile(
    addr: &Address,
    input: &[u8],
    gas_limit: u64,
) -> Result<(u64, Vec<u8>), VmError> {
    let result = match addr[19] {
        1 => precompile_ecrecover(input),
        2 => precompile_sha256(input),
        3 => precompile_ripemd160(input),
        4 => precompile_identity(input),
        _ => return Err(VmError::InvalidPrecompile),
    }?;
    if result.0 > gas_limit {
        return Err(VmError::OutOfGas);
    }
    Ok(result)
}

fn word_count(len: usize) -> Result<u64, VmError> {
    let words = len.checked_add(31).ok_or(VmError::Overflow)? / 32;
    u64::try_from(words).map_err(|_| VmError::Overflow)
}

fn precompile_ecrecover(input: &[u8]) -> Result<(u64, Vec<u8>), VmError> {
    let mut padded = [0u8; 128];
    padded[..input.len().min(128)].copy_from_slice(&input[..input.len().min(128)]);
    let mut output = vec![0u8; 32];
    let valid = padded[31] == 27 || padded[31] == 28;
    if !valid {
        return Ok((3_000, output));
    }
    let signature = match Signature::from_slice(&padded[64..128]) {
        Ok(signature) => signature,
        Err(_) => return Ok((3_000, output)),
    };
    let recovery_id = RecoveryId::try_from(padded[31] - 27).map_err(|_| VmError::Overflow)?;
    let key = match VerifyingKey::recover_from_prehash(&padded[..32], &signature, recovery_id) {
        Ok(key) => key,
        Err(_) => return Ok((3_000, output)),
    };
    let encoded = key.to_encoded_point(false);
    let hash = crate::crypto::keccak256(&encoded.as_bytes()[1..]);
    output[12..].copy_from_slice(&hash[12..]);
    Ok((3_000, output))
}

fn precompile_sha256(input: &[u8]) -> Result<(u64, Vec<u8>), VmError> {
    let words = word_count(input.len())?;
    let gas = 60u64
        .checked_add(words.checked_mul(12).ok_or(VmError::Overflow)?)
        .ok_or(VmError::Overflow)?;
    Ok((gas, Sha256::digest(input).to_vec()))
}

fn precompile_ripemd160(input: &[u8]) -> Result<(u64, Vec<u8>), VmError> {
    let words = word_count(input.len())?;
    let gas = 600u64
        .checked_add(words.checked_mul(120).ok_or(VmError::Overflow)?)
        .ok_or(VmError::Overflow)?;
    let digest = Ripemd160::digest(input);
    let mut output = vec![0u8; 32];
    output[12..].copy_from_slice(&digest);
    Ok((gas, output))
}

fn precompile_identity(input: &[u8]) -> Result<(u64, Vec<u8>), VmError> {
    let words = word_count(input.len())?;
    let gas = 15u64
        .checked_add(words.checked_mul(3).ok_or(VmError::Overflow)?)
        .ok_or(VmError::Overflow)?;
    Ok((gas, input.to_vec()))
}
