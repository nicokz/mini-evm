use ruint::aliases::U256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateAccount {
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: [u8; 32],
    pub code_hash: [u8; 32],
}

impl StateAccount {
    pub fn rlp_encode(&self) -> Vec<u8> {
        fn bytes(value: &[u8]) -> Vec<u8> {
            if value.len() == 1 && value[0] < 0x80 {
                return value.to_vec();
            }
            let mut result = vec![0x80 + value.len() as u8];
            result.extend_from_slice(value);
            result
        }
        fn integer(value: u64) -> Vec<u8> {
            if value == 0 {
                return bytes(&[]);
            }
            let raw = value.to_be_bytes();
            let first = raw.iter().position(|byte| *byte != 0).unwrap();
            bytes(&raw[first..])
        }
        let fields = [
            integer(self.nonce),
            {
                let raw = self.balance.to_be_bytes::<32>();
                let first = raw.iter().position(|byte| *byte != 0).unwrap_or(32);
                bytes(&raw[first..])
            },
            bytes(&self.storage_root),
            bytes(&self.code_hash),
        ];
        let payload_len: usize = fields.iter().map(Vec::len).sum();
        let mut result = if payload_len <= 55 {
            vec![0xc0 + payload_len as u8]
        } else {
            let length = payload_len.to_be_bytes();
            let first = length.iter().position(|byte| *byte != 0).unwrap();
            let length_bytes = &length[first..];
            let mut prefix = vec![0xf7 + length_bytes.len() as u8];
            prefix.extend_from_slice(length_bytes);
            prefix
        };
        result.extend(fields.into_iter().flatten());
        result
    }
}
