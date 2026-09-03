use crate::env::Address;
use ruint::aliases::U256;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountState {
    pub nonce: u64,
    pub balance: U256,
    pub code: Vec<u8>,
    pub storage: HashMap<U256, U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_id: usize,
    pub dirty_state: HashMap<Address, AccountState>,
    pub transient_storage: HashMap<(Address, U256), U256>,
}

#[derive(Debug, Clone, Default)]
pub struct StateFork {
    pub base_state: HashMap<Address, AccountState>,
    pub dirty_state: HashMap<Address, AccountState>,
    pub snapshots: Vec<Snapshot>,
    pub transient_storage: HashMap<(Address, U256), U256>,
}

impl StateFork {
    pub fn snapshot(&mut self) -> usize {
        let snapshot_id = self.snapshots.len();
        self.snapshots.push(Snapshot {
            snapshot_id,
            dirty_state: self.dirty_state.clone(),
            transient_storage: self.transient_storage.clone(),
        });
        snapshot_id
    }

    pub fn revert_to_snapshot(&mut self, id: usize) {
        if let Some(position) = self
            .snapshots
            .iter()
            .position(|snapshot| snapshot.snapshot_id == id)
        {
            self.dirty_state = self.snapshots[position].dirty_state.clone();
            self.transient_storage = self.snapshots[position].transient_storage.clone();
            self.snapshots.truncate(position);
        }
    }

    pub fn get_account(&self, addr: &Address) -> AccountState {
        self.dirty_state
            .get(addr)
            .or_else(|| self.base_state.get(addr))
            .cloned()
            .unwrap_or_default()
    }

    pub fn tload(&self, addr: &Address, key: &U256) -> U256 {
        self.transient_storage
            .get(&(*addr, *key))
            .copied()
            .unwrap_or(U256::ZERO)
    }

    pub fn tstore(&mut self, addr: Address, key: U256, value: U256) {
        self.transient_storage.insert((addr, key), value);
    }

    pub fn clear_transient_storage(&mut self) {
        self.transient_storage.clear();
    }

    pub fn get_storage(&self, addr: &Address, slot: U256) -> U256 {
        self.get_account(addr)
            .storage
            .get(&slot)
            .copied()
            .unwrap_or(U256::ZERO)
    }

    pub fn get_storage_at(&self, addr: &Address, slot: &U256) -> U256 {
        self.get_storage(addr, *slot)
    }

    pub fn set_storage(&mut self, addr: Address, slot: U256, value: U256) {
        self.account_for_write(addr).storage.insert(slot, value);
    }

    pub fn get_balance(&self, addr: &Address) -> U256 {
        self.get_account(addr).balance
    }

    pub fn set_balance(&mut self, addr: Address, balance: U256) {
        self.account_for_write(addr).balance = balance;
    }

    pub fn get_code(&self, addr: &Address) -> Vec<u8> {
        self.get_account(addr).code
    }

    pub fn set_code(&mut self, addr: Address, code: Vec<u8>) {
        self.account_for_write(addr).code = code;
    }

    pub fn set_nonce(&mut self, addr: Address, nonce: u64) {
        self.account_for_write(addr).nonce = nonce;
    }

    fn account_for_write(&mut self, addr: Address) -> &mut AccountState {
        if !self.dirty_state.contains_key(&addr) {
            let account = self.base_state.get(&addr).cloned().unwrap_or_default();
            self.dirty_state.insert(addr, account);
        }
        self.dirty_state.get_mut(&addr).expect("account inserted")
    }
}
