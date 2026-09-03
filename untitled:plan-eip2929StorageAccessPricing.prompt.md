Under EIP-2929, storage access pricing splits into **cold** (first access in a transaction) and **warm** (subsequent accesses). Cold reads pay 2,100 gas, while warm reads cost only 100 gas. For `SSTORE`, a 2,100 cold surcharge is added on top of the base state-mutation cost.

Because these opcodes now depend on transaction state, set their entries in `gas_table` to `0` and manage gas deduction dynamically inside the opcode handlers.

### 1. Update `Evm` struct and gas table (`src/vm.rs`)
Add `accessed_slots` to track touched storage keys:

```rust
use std::collections::{HashMap, HashSet};

pub struct Evm {
    pub code: Vec<u8>,
    pub pc: usize,
    pub stack: Stack,
    pub memory: Vec<u8>,
    pub storage: HashMap<u128, u128>,
    pub accessed_slots: HashSet<u128>, // EIP-2929 warm storage set
    pub context: TxContext,
    pub valid_jumpdests: Vec<bool>,
    pub gas_left: u64,
    dispatch_table: [OpFn; 256],
    gas_table: [u64; 256],
}
```

Zero out `SLOAD` and `SSTORE` in `build_gas_table()` so static billing doesn't double-charge:

```rust
// Handled dynamically in op_sload and op_sstore
table[SLOAD as usize] = 0;
table[SSTORE as usize] = 0;
```

Initialize `accessed_slots` in `Evm::new_with_gas`:

```rust
Self {
    code: code.to_vec(),
    pc: 0,
    stack: Stack::new(),
    memory: Vec::new(),
    storage: HashMap::new(),
    accessed_slots: HashSet::new(),
    context: TxContext::default(),
    valid_jumpdests,
    gas_left: gas_limit,
    dispatch_table,
    gas_table: build_gas_table(),
}
```

### 2. Add warm check helper (`src/vm.rs`)

```rust
impl Evm {
    /// Returns `true` if key was already warm, `false` if it was cold (and marks it warm).
    #[inline]
    pub fn mark_slot_warm(&mut self, key: u128) -> bool {
        !self.accessed_slots.insert(key)
    }
}
```

### 3. Update `op_sload` and `op_sstore` (`src/vm.rs`)

```rust
fn op_sload(evm: &mut Evm) -> ExecutionResult {
    let key = match evm.stack.pop() {
        Ok(k) => k as u128,
        Err(e) => return ExecutionResult::Error(e),
    };

    let gas_cost = if evm.mark_slot_warm(key) {
        100 // Warm read
    } else {
        2_100 // Cold read
    };

    if evm.gas_left < gas_cost {
        return ExecutionResult::OutOfGas;
    }
    evm.gas_left -= gas_cost;

    let val = evm.storage.get(&key).copied().unwrap_or(0) as u64;
    if evm.stack.push(val).is_err() {
        return ExecutionResult::Error("Stack Overflow on SLOAD");
    }
    ExecutionResult::Halt
}

fn op_sstore(evm: &mut Evm) -> ExecutionResult {
    let (key, val) = match (evm.stack.pop(), evm.stack.pop()) {
        (Ok(k), Ok(v)) => (k as u128, v as u128),
        _ => return ExecutionResult::Error("Stack Underflow on SSTORE"),
    };

    let is_warm = evm.mark_slot_warm(key);
    let cold_surcharge = if is_warm { 0 } else { 2_100 };

    let current_val = evm.storage.get(&key).copied().unwrap_or(0);
    let base_cost = if current_val == val {
        100 // No-op rewrite
    } else if current_val == 0 {
        20_000 // Fresh slot creation
    } else {
        2_900 // Update existing non-zero slot
    };

    let total_cost = base_cost + cold_surcharge;

    if evm.gas_left < total_cost {
        return ExecutionResult::OutOfGas;
    }
    evm.gas_left -= total_cost;

    evm.storage.insert(key, val);
    ExecutionResult::Halt
}
```

### 4. Unit Test in `src/lib.rs`

```rust
#[test]
fn test_eip2929_warm_cold_sload() {
    // PUSH1 1 (key), SLOAD (cold -> 2,100 gas), PUSH1 1 (key), SLOAD (warm -> 100 gas)
    // PUSH1 cost = 3 * 2 = 6 gas. Total expected gas = 2,100 + 100 + 6 = 2,206 gas.
    let code = vec![PUSH1, 0x01, SLOAD, PUSH1, 0x01, SLOAD];

    let mut vm = Evm::new_with_gas(&code, 5_000);
    assert_eq!(vm.run(), ExecutionResult::Halt);
    assert_eq!(vm.gas_left, 5_000 - 2_206);
    assert!(vm.accessed_slots.contains(&1));
}
```