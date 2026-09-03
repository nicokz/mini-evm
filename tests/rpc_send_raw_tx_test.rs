#[cfg(test)]
mod tests {
    use mini_evm::tx::decoder::decode_raw_tx;
    use primitive_types::H160;

    #[test]
    fn test_decode_and_execute_raw_tx() {
        // Canonical EIP-155 example signed with chain ID 1.
        let raw_tx_hex = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

        let bytes = hex::decode(raw_tx_hex).unwrap();
        let tx = decode_raw_tx(&bytes).expect("Failed to decode signed raw tx");

        assert_eq!(tx.nonce, 9);
        assert_eq!(tx.gas_limit, 21000);
        assert_eq!(
            tx.sender,
            "0x9d8a62f656a8d1615c1294fd71e9cfb3e4855a4f"
                .parse::<H160>()
                .unwrap()
        );
    }
}
