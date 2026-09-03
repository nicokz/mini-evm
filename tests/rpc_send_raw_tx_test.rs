#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::{H160, U256};
    use crate::tx::decoder::decode_raw_tx;

    #[test]
    fn test_decode_and_execute_raw_tx() {
        // Pre-calculated Legacy Tx signed by test key 0x01...
        // Sender: 0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf
        let raw_tx_hex = "f86c808504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028cc68860495f236e86417b0879b69666b6c6c54fe0028132b34b52538b4b16aa047465f24772d16c41d5d2699888c0a952f365f807d9408a04506dedf65640730";
        
        let bytes = hex::decode(raw_tx_hex).unwrap();
        let tx = decode_raw_tx(&bytes).expect("Failed to decode signed raw tx");

        assert_eq!(tx.nonce, 0);
        assert_eq!(tx.gas_limit, 21000);
        assert_eq!(
            tx.sender,
            "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf".parse::<H160>().unwrap()
        );
    }
}