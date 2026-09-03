use crate::env::Address;
use crate::tx::decoder::SignedTransaction;
use primitive_types::U256 as PrimitiveU256;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct PendingTx {
    pub raw: Vec<u8>,
    pub tx: SignedTransaction,
    pub sender: Address,
    pub nonce: u64,
    pub max_priority_fee_per_gas: PrimitiveU256,
    pub gas_limit: u64,
}

#[derive(Debug, Default)]
pub struct TxPool {
    by_sender: Mutex<HashMap<Address, BTreeMap<u64, PendingTx>>>,
    min_tip: PrimitiveU256,
}

impl TxPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_tip(min_tip: PrimitiveU256) -> Self {
        Self {
            by_sender: Mutex::new(HashMap::new()),
            min_tip,
        }
    }

    pub fn insert(
        &self,
        sender: Address,
        tx: SignedTransaction,
        raw: Vec<u8>,
    ) -> Result<(), String> {
        if tx.max_priority_fee_per_gas < self.min_tip {
            return Err("transaction tip is below the pool minimum".into());
        }
        let mut pool = self
            .by_sender
            .lock()
            .map_err(|_| "transaction pool poisoned")?;
        let transactions = pool.entry(sender).or_default();
        if transactions.contains_key(&tx.nonce) {
            return Err("replacement transactions are not supported".into());
        }

        transactions.insert(
            tx.nonce,
            PendingTx {
                raw,
                sender,
                nonce: tx.nonce,
                max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                gas_limit: tx.gas_limit,
                tx,
            },
        );
        Ok(())
    }

    pub fn senders(&self) -> Result<Vec<Address>, String> {
        let pool = self
            .by_sender
            .lock()
            .map_err(|_| "transaction pool poisoned")?;
        Ok(pool.keys().copied().collect())
    }

    pub fn pop_executable(
        &self,
        current_nonces: &HashMap<Address, u64>,
        block_gas_limit: u64,
    ) -> Result<Vec<PendingTx>, String> {
        let mut pool = self
            .by_sender
            .lock()
            .map_err(|_| "transaction pool poisoned")?;
        let mut active_nonces = current_nonces.clone();
        let mut selected = Vec::new();
        let mut gas_used = 0u64;

        loop {
            let mut best: Option<(Address, u64, PrimitiveU256)> = None;
            for (sender, transactions) in &*pool {
                let expected_nonce = active_nonces.get(sender).copied().unwrap_or_default();
                let Some((&nonce, transaction)) = transactions.iter().next() else {
                    continue;
                };
                if nonce != expected_nonce
                    || gas_used.saturating_add(transaction.gas_limit) > block_gas_limit
                {
                    continue;
                }

                let is_better = best
                    .as_ref()
                    .map(|(_, _, tip)| transaction.max_priority_fee_per_gas > *tip)
                    .unwrap_or(true);
                if is_better {
                    best = Some((*sender, nonce, transaction.max_priority_fee_per_gas));
                }
            }

            let Some((sender, nonce, _)) = best else {
                break;
            };
            let transactions = pool.get_mut(&sender).expect("selected sender exists");
            let transaction = transactions.remove(&nonce).expect("selected nonce exists");
            if transactions.is_empty() {
                pool.remove(&sender);
            }
            gas_used = gas_used.saturating_add(transaction.gas_limit);
            active_nonces.insert(sender, nonce.saturating_add(1));
            selected.push(transaction);
        }

        Ok(selected)
    }

    pub fn len(&self) -> Result<usize, String> {
        let pool = self
            .by_sender
            .lock()
            .map_err(|_| "transaction pool poisoned")?;
        Ok(pool.values().map(BTreeMap::len).sum())
    }

    pub fn is_empty(&self) -> Result<bool, String> {
        Ok(self.len()? == 0)
    }

    pub fn pending(&self) -> Result<Vec<PendingTx>, String> {
        let pool = self
            .by_sender
            .lock()
            .map_err(|_| "transaction pool poisoned")?;
        Ok(pool
            .values()
            .flat_map(|transactions| transactions.values().cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::H256;

    fn transaction(nonce: u64, tip: u64, gas_limit: u64) -> SignedTransaction {
        SignedTransaction {
            hash: H256::zero(),
            tx_type: crate::tx::decoder::TxType::Legacy,
            chain_id: Some(1),
            nonce,
            gas_limit,
            max_fee_per_gas: PrimitiveU256::from(tip),
            max_priority_fee_per_gas: PrimitiveU256::from(tip),
            to: None,
            value: PrimitiveU256::zero(),
            data: Vec::new(),
            sender: primitive_types::H160::zero(),
        }
    }

    #[test]
    fn selects_sequential_nonces_and_highest_tip() {
        let pool = TxPool::new();
        let first_sender = [1; 20];
        let second_sender = [2; 20];
        pool.insert(first_sender, transaction(0, 1, 21_000), vec![])
            .unwrap();
        pool.insert(first_sender, transaction(1, 100, 21_000), vec![])
            .unwrap();
        pool.insert(second_sender, transaction(0, 10, 21_000), vec![])
            .unwrap();

        let mut nonces = HashMap::new();
        nonces.insert(first_sender, 0);
        nonces.insert(second_sender, 0);
        let selected = pool.pop_executable(&nonces, 63_000).unwrap();

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].sender, second_sender);
        assert_eq!(selected[1].sender, first_sender);
        assert_eq!(selected[1].nonce, 0);
        assert_eq!(selected[2].nonce, 1);
    }

    #[test]
    fn does_not_replace_same_sender_nonce() {
        let pool = TxPool::new();
        let sender = [1; 20];
        pool.insert(sender, transaction(0, 1, 21_000), vec![])
            .unwrap();
        assert!(
            pool.insert(sender, transaction(0, 2, 21_000), vec![])
                .is_err()
        );
    }
}
