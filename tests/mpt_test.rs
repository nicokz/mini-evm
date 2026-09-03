use mini_evm::mpt::MerklePatriciaTrie;
use mini_evm::opcodes::*;
use mini_evm::vm::Evm;
use ruint::aliases::U256;

#[test]
fn test_empty_trie_root() {
    let trie = MerklePatriciaTrie::new();
    assert_eq!(
        trie.root_hash(),
        [
            0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
            0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
            0xe3, 0x63, 0xb4, 0x21,
        ]
    );
}

#[test]
fn test_known_mpt_insertions() {
    let mut trie = MerklePatriciaTrie::new();
    trie.insert(&[0x12], b"first".to_vec());
    let first_root = trie.root_hash();
    assert_eq!(trie.get(&[0x12]), Some(b"first".to_vec()));

    trie.insert(&[0x13], b"second".to_vec());
    assert_eq!(trie.get(&[0x12]), Some(b"first".to_vec()));
    assert_eq!(trie.get(&[0x13]), Some(b"second".to_vec()));
    assert_ne!(trie.root_hash(), first_root);
}

#[test]
fn test_state_root_reflects_sstore_changes() {
    let mut vm = Evm::new(&[PUSH1, 42, PUSH1, 1, SSTORE]);
    let before = vm.compute_state_root();
    assert_eq!(vm.run(), mini_evm::vm::ExecutionResult::Halt);
    let after = vm.compute_state_root();

    assert_ne!(before, after);
    assert_eq!(vm.storage.get(&U256::from(1u8)), Some(&U256::from(42u8)));
}
