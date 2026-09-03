// src/tx/decoder.rs

use primitive_types::{H160, H256, U256};
use rlp::{Rlp, RlpStream};
use secp256k1::{Message, Secp256k1, ecdsa::RecoverableSignature, ecdsa::RecoveryId};
use tiny_keccak::{Hasher, Keccak};
//pub use crate::tx::decoder::{decode_raw_tx, SignedTransaction, TxDecodeError, TxType};
pub type Address = H160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxType {
    Legacy,
    Eip2930,
    Eip1559,
}

#[derive(Debug, Clone)]
pub struct SignedTransaction {
    pub hash: H256,
    pub tx_type: TxType,
    pub chain_id: Option<u64>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Vec<u8>,
    pub sender: Address,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TxDecodeError {
    InvalidRlp,
    UnsupportedType,
    InvalidSignature,
    RecoveryFailed,
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    let mut out = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut out);
    out
}

pub fn decode_raw_tx(raw_bytes: &[u8]) -> Result<SignedTransaction, TxDecodeError> {
    if raw_bytes.is_empty() {
        return Err(TxDecodeError::InvalidRlp);
    }

    let tx_hash = H256::from(keccak256(raw_bytes));

    // Check envelope prefix
    match raw_bytes[0] {
        0x02 => decode_eip1559(&raw_bytes[1..], tx_hash),
        first if first >= 0xc0 => decode_legacy(raw_bytes, tx_hash),
        _ => Err(TxDecodeError::UnsupportedType),
    }
}

fn decode_eip1559(payload: &[u8], tx_hash: H256) -> Result<SignedTransaction, TxDecodeError> {
    let rlp = Rlp::new(payload);
    if !rlp.is_list() || rlp.item_count()? < 12 {
        return Err(TxDecodeError::InvalidRlp);
    }

    let chain_id: u64 = rlp.val_at(0)?;
    let nonce: u64 = rlp.val_at(1)?;
    let max_priority_fee_per_gas: U256 = rlp.val_at(2)?;
    let max_fee_per_gas: U256 = rlp.val_at(3)?;
    let gas_limit: u64 = rlp.val_at(4)?;

    let to_bytes: Vec<u8> = rlp.val_at(5)?;
    let to = if to_bytes.is_empty() {
        None
    } else {
        Some(Address::from_slice(&to_bytes))
    };

    let value: U256 = rlp.val_at(6)?;
    let data: Vec<u8> = rlp.val_at(7)?;
    // item 8 is access_list, skipping parsing logic for brevity

    let y_parity: u8 = rlp.val_at(9)?;
    let r_bytes: Vec<u8> = rlp.val_at(10)?;
    let s_bytes: Vec<u8> = rlp.val_at(11)?;

    // Reconstruct unsigned payload for sighash computation: 0x02 || RLP([chain_id, nonce, ...])
    let mut stream = RlpStream::new_list(9);
    stream.append(&chain_id);
    stream.append(&nonce);
    stream.append(&max_priority_fee_per_gas);
    stream.append(&max_fee_per_gas);
    stream.append(&gas_limit);
    if let Some(ref addr) = to {
        stream.append(&addr.as_bytes().to_vec());
    } else {
        stream.append(&"");
    }
    stream.append(&value);
    stream.append(&data);
    stream.append_list::<Vec<u8>, Vec<u8>>(&[]);

    let mut unsigned_payload = vec![0x02];
    unsigned_payload.extend_from_slice(&stream.out());
    let sighash = keccak256(&unsigned_payload);

    let sender = recover_sender(&sighash, &r_bytes, &s_bytes, y_parity)?;

    Ok(SignedTransaction {
        hash: tx_hash,
        tx_type: TxType::Eip1559,
        chain_id: Some(chain_id),
        nonce,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        to,
        value,
        data,
        sender,
    })
}

fn decode_legacy(raw_bytes: &[u8], tx_hash: H256) -> Result<SignedTransaction, TxDecodeError> {
    let rlp = Rlp::new(raw_bytes);
    if !rlp.is_list() || rlp.item_count()? < 9 {
        return Err(TxDecodeError::InvalidRlp);
    }

    let nonce: u64 = rlp.val_at(0)?;
    let gas_price: U256 = rlp.val_at(1)?;
    let gas_limit: u64 = rlp.val_at(2)?;

    let to_bytes: Vec<u8> = rlp.val_at(3)?;
    let to = if to_bytes.is_empty() {
        None
    } else {
        Some(Address::from_slice(&to_bytes))
    };

    let value: U256 = rlp.val_at(4)?;
    let data: Vec<u8> = rlp.val_at(5)?;
    let v: u64 = rlp.val_at(6)?;
    let r_bytes: Vec<u8> = rlp.val_at(7)?;
    let s_bytes: Vec<u8> = rlp.val_at(8)?;

    // Derive chain_id and recovery parity from v (EIP-155 check)
    let (chain_id, rec_id) = if v >= 35 {
        let chain_id = (v - 35) / 2;
        let rec_id = ((v - 35) % 2) as u8;
        (Some(chain_id), rec_id)
    } else if v == 27 || v == 28 {
        (None, (v - 27) as u8)
    } else {
        return Err(TxDecodeError::InvalidSignature);
    };

    // Reconstruct sighash
    let mut stream = RlpStream::new();
    if let Some(cid) = chain_id {
        stream.begin_list(9);
        stream.append(&nonce);
        stream.append(&gas_price);
        stream.append(&gas_limit);
        if let Some(ref addr) = to {
            stream.append(&addr.as_bytes().to_vec());
        } else {
            stream.append(&"");
        }
        stream.append(&value);
        stream.append(&data);
        stream.append(&cid);
        stream.append_empty_data();
        stream.append_empty_data();
    } else {
        stream.begin_list(6);
        stream.append(&nonce);
        stream.append(&gas_price);
        stream.append(&gas_limit);
        if let Some(ref addr) = to {
            stream.append(&addr.as_bytes());
        } else {
            stream.append(&"");
        }
        stream.append(&value);
        stream.append(&data);
    }

    let sighash = keccak256(&stream.out());
    let sender = recover_sender(&sighash, &r_bytes, &s_bytes, rec_id)?;

    Ok(SignedTransaction {
        hash: tx_hash,
        tx_type: TxType::Legacy,
        chain_id,
        nonce,
        gas_limit,
        max_fee_per_gas: gas_price,
        max_priority_fee_per_gas: gas_price,
        to,
        value,
        data,
        sender,
    })
}

fn recover_sender(
    sighash: &[u8; 32],
    r: &[u8],
    s: &[u8],
    rec_id: u8,
) -> Result<Address, TxDecodeError> {
    let mut sig_bytes = [0u8; 64];
    let r_padded = pad_zero_left(r, 32);
    let s_padded = pad_zero_left(s, 32);
    sig_bytes[..32].copy_from_slice(&r_padded);
    sig_bytes[32..].copy_from_slice(&s_padded);

    let recovery_id =
        RecoveryId::from_i32(rec_id as i32).map_err(|_| TxDecodeError::InvalidSignature)?;
    let sig = RecoverableSignature::from_compact(&sig_bytes, recovery_id)
        .map_err(|_| TxDecodeError::InvalidSignature)?;

    let msg = Message::from_digest(*sighash);
    let secp = Secp256k1::verification_only();
    let pub_key = secp
        .recover_ecdsa(&msg, &sig)
        .map_err(|_| TxDecodeError::RecoveryFailed)?;

    // Ethereum address = last 20 bytes of Keccak256(uncompressed_pubkey[1..])
    let uncompressed = pub_key.serialize_uncompressed();
    let pub_hash = keccak256(&uncompressed[1..]);
    Ok(Address::from_slice(&pub_hash[12..]))
}

fn pad_zero_left(slice: &[u8], len: usize) -> Vec<u8> {
    if slice.len() >= len {
        return slice[slice.len() - len..].to_vec();
    }
    let mut padded = vec![0u8; len - slice.len()];
    padded.extend_from_slice(slice);
    padded
}

impl From<rlp::DecoderError> for TxDecodeError {
    fn from(_: rlp::DecoderError) -> Self {
        TxDecodeError::InvalidRlp
    }
}
