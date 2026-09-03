use crate::env::Address;
use ruint::aliases::U256;
use tiny_keccak::{Hasher, Keccak};

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut keccak = Keccak::v256();
    keccak.update(data);
    keccak.finalize(&mut hash);
    hash
}

fn encode_nonce(nonce: u64) -> Vec<u8> {
    if nonce == 0 {
        return vec![0x80];
    }
    let bytes = nonce.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap();
    let value = &bytes[first..];
    if value.len() == 1 && value[0] < 0x80 {
        value.to_vec()
    } else {
        let mut encoded = vec![0x80 + value.len() as u8];
        encoded.extend_from_slice(value);
        encoded
    }
}

pub fn rlp_encode_create(sender: &[u8; 20], nonce: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(1 + 20 + 9);
    payload.push(0x94);
    payload.extend_from_slice(sender);
    payload.extend_from_slice(&encode_nonce(nonce));

    let mut rlp = Vec::with_capacity(1 + payload.len());
    rlp.push(0xc0 + payload.len() as u8);
    rlp.extend_from_slice(&payload);
    rlp
}

pub fn derive_create_address(sender: &[u8; 20], nonce: u64) -> Address {
    let hash = keccak256(&rlp_encode_create(sender, nonce));
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

pub fn derive_create2_address(sender: &[u8; 20], salt: U256, init_code: &[u8]) -> Address {
    let init_code_hash = keccak256(init_code);
    let mut buffer = Vec::with_capacity(85);
    buffer.push(0xff);
    buffer.extend_from_slice(sender);
    buffer.extend_from_slice(&salt.to_be_bytes::<32>());
    buffer.extend_from_slice(&init_code_hash);

    let hash = keccak256(&buffer);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}
