use crate::call_frame::{CallContext, CallFrame};
use crate::crypto::{derive_create_address, derive_create2_address};
use crate::disasm::opcode_mnemonic;
pub use crate::env::{Address, BlockHashFn, Environment};
use crate::gas::calc_eip150_call_gas;
use crate::log::LogRecord;
use crate::mpt::{MerklePatriciaTrie, StateAccount};
use crate::opcodes::*;
use crate::precompiles::{execute_precompile, is_precompile};
use crate::stack::Stack;
use crate::state::StateFork;
use crate::tracer::{StepFrame, StepTracer};
use ruint::aliases::U256;
use std::collections::{HashMap, HashSet};

#[path = "vm/runner.rs"]
pub mod runner;
use tiny_keccak::{Hasher, Keccak};

#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionResult {
    Halt,
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Error(&'static str),
    VmError(VmError),
    OutOfGas,
    StackUnderflow,
    StackOverflow,
    InvalidOpcode(u8),
}

type OpFn = fn(&mut Evm) -> ExecutionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    Overflow,
    OutOfBoundsReturnData,
    StaticCallViolation,
    OutOfGas,
    InvalidPrecompile,
}

fn calc_copy_gas(size_in_bytes: U256) -> Result<u64, VmError> {
    let words = size_in_bytes
        .checked_add(U256::from(31u8))
        .ok_or(VmError::Overflow)?
        / U256::from(32u8);
    let words: u64 = words.try_into().map_err(|_| VmError::Overflow)?;
    words.checked_mul(3).ok_or(VmError::Overflow)
}

#[inline]
fn as_usize(value: U256) -> usize {
    value.as_limbs()[0] as usize
}

fn build_gas_table() -> [u64; 256] {
    let mut table = [0u64; 256];

    table[STOP as usize] = 0;
    table[ADD as usize] = 3;
    table[MUL as usize] = 5;
    table[SUB as usize] = 3;
    table[DIV as usize] = 5;
    table[SDIV as usize] = 5;
    table[MOD as usize] = 5;
    table[SMOD as usize] = 5;
    table[ADDMOD as usize] = 8;
    table[MULMOD as usize] = 8;
    table[EXP as usize] = 10;
    table[SIGNEXTEND as usize] = 5;
    table[KECCAK256 as usize] = 30;
    table[LT as usize] = 3;
    table[GT as usize] = 3;
    table[EQ as usize] = 3;
    table[ISZERO as usize] = 3;
    table[AND as usize] = 3;
    table[OR as usize] = 3;
    table[XOR as usize] = 3;
    table[NOT as usize] = 3;
    table[BYTE as usize] = 3;
    table[SHL as usize] = 3;
    table[SHR as usize] = 3;
    table[SAR as usize] = 3;
    table[ADDRESS as usize] = 2;
    table[BALANCE as usize] = 0;
    table[BLOCKHASH as usize] = 20;
    table[SELFBALANCE as usize] = 5;
    table[CALLER as usize] = 2;
    table[CALLVALUE as usize] = 2;
    table[CALLDATALOAD as usize] = 3;
    table[CALLDATASIZE as usize] = 2;
    table[CALLDATACOPY as usize] = 3;
    table[CODECOPY as usize] = 3;
    table[CODESIZE as usize] = 2;
    table[GASPRICE as usize] = 2;
    table[ORIGIN as usize] = 2;
    table[COINBASE as usize] = 2;
    table[TIMESTAMP as usize] = 2;
    table[NUMBER as usize] = 2;
    table[PREVRANDAO as usize] = 2;
    table[GASLIMIT as usize] = 2;
    table[CHAINID as usize] = 2;
    table[BASEFEE as usize] = 2;
    table[BLOBHASH as usize] = 3;
    table[BLOBBASEFEE as usize] = 2;
    table[GAS as usize] = 2;
    table[RETURNDATASIZE as usize] = 2;
    table[RETURNDATACOPY as usize] = 3;
    table[CALL as usize] = 0;
    table[CREATE as usize] = 0;
    table[DELEGATECALL as usize] = 0;
    table[CREATE2 as usize] = 0;
    table[STATICCALL as usize] = 0;
    table[INVALID as usize] = 0;
    table[SELFDESTRUCT as usize] = 0;
    table[MLOAD as usize] = 3;
    table[MSTORE as usize] = 3;
    table[MSTORE8 as usize] = 3;
    table[PUSH1 as usize] = 3;
    table[PUSH2 as usize] = 3;
    table[PUSH3 as usize..=PUSH32 as usize].fill(3);
    table[MSIZE as usize] = 2;
    table[MCOPY as usize] = 0;
    table[JUMPDEST as usize] = 1;

    table[JUMP as usize] = 8;
    table[JUMPI as usize] = 10;

    // Handled dynamically in op_sload and op_sstore
    table[SLOAD as usize] = 0;
    table[SSTORE as usize] = 0;
    table[TLOAD as usize] = 100;
    table[TSTORE as usize] = 100;

    table[0x80..=0x9F].fill(3);

    table
}

#[derive(Debug, Clone, Default)]
pub struct TxContext {
    pub caller: [u8; 20],
    pub address: [u8; 20],
    pub value: u128,
    pub calldata: Vec<u8>,
    pub blob_versioned_hashes: Vec<[u8; 32]>,
}

#[derive(Debug, Clone)]
pub struct EvmBuilder {
    code: Vec<u8>,
    tx_context: TxContext,
    block_context: Environment,
    gas_limit: u64,
}

impl Default for EvmBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EvmBuilder {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            tx_context: TxContext::default(),
            block_context: Environment::default(),
            gas_limit: 10_000_000,
        }
    }

    pub fn with_code(mut self, code: impl Into<Vec<u8>>) -> Self {
        self.code = code.into();
        self
    }

    pub fn with_gas(mut self, gas_limit: u64) -> Self {
        self.gas_limit = gas_limit;
        self
    }

    pub fn with_blob_hashes(mut self, blob_hashes: Vec<[u8; 32]>) -> Self {
        self.tx_context.blob_versioned_hashes = blob_hashes;
        self
    }

    pub fn with_blob_base_fee(mut self, blob_base_fee: U256) -> Self {
        self.block_context.blob_base_fee = blob_base_fee;
        self
    }

    pub fn build(self) -> Evm {
        let mut evm = Evm::new_with_gas(&self.code, self.gas_limit);
        evm.context = self.tx_context;
        evm.env = self.block_context;
        evm
    }
}

macro_rules! impl_dup_swap_handlers {
    (
        dup: $( ($n:literal, $dup_fn:ident) ),+ ;
        swap: $( ($sn:literal, $swap_fn:ident) ),+
    ) => {
        $(
            fn $dup_fn(evm: &mut Evm) -> ExecutionResult {
                let val = match evm.stack.peek($n) {
                    Ok(v) => v,
                    Err(e) => return ExecutionResult::Error(e),
                };
                if evm.stack.push(val).is_err() {
                    return ExecutionResult::Error(concat!("Stack Overflow on DUP", stringify!($n)));
                }
                ExecutionResult::Halt
            }
        )+

        $(
            fn $swap_fn(evm: &mut Evm) -> ExecutionResult {
                if let Err(e) = evm.stack.swap($sn) {
                    return ExecutionResult::Error(e);
                }
                ExecutionResult::Halt
            }
        )+
    };
}

macro_rules! register_ops {
    ($table:expr, $( $op:ident => $fn:ident ),+ $(,)?) => {
        $(
            $table[$op as usize] = Self::$fn;
        )+
    };
}

macro_rules! impl_push_n {
    ($($name:ident => $n:literal),+ $(,)?) => {
        $(
            fn $name(evm: &mut Evm) -> ExecutionResult {
                Evm::execute_push_n(evm, $n)
            }
        )+
    };
}

macro_rules! register_push_n {
    ($table:expr, $(($opcode:ident, $handler:ident, $n:literal)),+ $(,)?) => {
        $(
            $table[$opcode as usize] = Evm::$handler;
        )+
    };
}

macro_rules! impl_log_n {
    ($(($name:ident, $topics:literal)),+ $(,)?) => {
        $(
            fn $name(evm: &mut Evm) -> ExecutionResult {
                Evm::op_log(evm, $topics)
            }
        )+
    };
}

pub struct Evm {
    pub code: Vec<u8>,
    pub pc: usize,
    pub stack: Stack,
    pub memory: Vec<u8>,
    pub storage: HashMap<U256, U256>,
    pub balances: HashMap<Address, U256>,
    pub nonces: HashMap<Address, u64>,
    pub contracts: HashMap<Address, Vec<u8>>,
    pub accessed_addresses: HashSet<Address>,
    pub accessed_slots: HashSet<U256>,
    pub context: TxContext,
    pub storage_address: Address,
    pub transient_storage_address: Option<Address>,
    pub env: Environment,
    pub return_data: Vec<u8>,
    pub logs: Vec<LogRecord>,
    pub is_static: bool,
    pub valid_jumpdests: Vec<bool>,
    pub gas_left: u64,
    pub tracer: Option<StepTracer>,
    pub state: StateFork,
    pub created_in_transaction: HashSet<Address>,
    dispatch_table: [OpFn; 256],
    gas_table: [u64; 256],
}

impl Evm {
    pub fn new(code: &[u8]) -> Self {
        Self::new_with_gas(code, 10_000_000)
    }

    pub fn new_with_gas(code: &[u8], gas_limit: u64) -> Self {
        let mut dispatch_table: [OpFn; 256] = [Self::op_invalid; 256];
        let valid_jumpdests = Self::analyze_jumpdests(code);

        // Register opcodes
        dispatch_table[STOP as usize] = Self::op_stop;
        dispatch_table[ADD as usize] = Self::op_add;
        dispatch_table[MUL as usize] = Self::op_mul;
        dispatch_table[SUB as usize] = Self::op_sub;
        dispatch_table[DIV as usize] = Self::op_div;
        dispatch_table[SDIV as usize] = Self::op_sdiv;
        dispatch_table[MOD as usize] = Self::op_mod;
        dispatch_table[SMOD as usize] = Self::op_smod;
        dispatch_table[ADDMOD as usize] = Self::op_addmod;
        dispatch_table[MULMOD as usize] = Self::op_mulmod;
        dispatch_table[EXP as usize] = Self::op_exp;
        dispatch_table[SIGNEXTEND as usize] = Self::op_signextend;
        dispatch_table[KECCAK256 as usize] = Self::op_keccak256;
        dispatch_table[PUSH1 as usize] = Self::op_push1;
        dispatch_table[PUSH2 as usize] = Self::op_push2;
        register_push_n!(
            dispatch_table,
            (PUSH3, op_push3, 3),
            (PUSH4, op_push4, 4),
            (PUSH5, op_push5, 5),
            (PUSH6, op_push6, 6),
            (PUSH7, op_push7, 7),
            (PUSH8, op_push8, 8),
            (PUSH9, op_push9, 9),
            (PUSH10, op_push10, 10),
            (PUSH11, op_push11, 11),
            (PUSH12, op_push12, 12),
            (PUSH13, op_push13, 13),
            (PUSH14, op_push14, 14),
            (PUSH15, op_push15, 15),
            (PUSH16, op_push16, 16),
            (PUSH17, op_push17, 17),
            (PUSH18, op_push18, 18),
            (PUSH19, op_push19, 19),
            (PUSH20, op_push20, 20),
            (PUSH21, op_push21, 21),
            (PUSH22, op_push22, 22),
            (PUSH23, op_push23, 23),
            (PUSH24, op_push24, 24),
            (PUSH25, op_push25, 25),
            (PUSH26, op_push26, 26),
            (PUSH27, op_push27, 27),
            (PUSH28, op_push28, 28),
            (PUSH29, op_push29, 29),
            (PUSH30, op_push30, 30),
            (PUSH31, op_push31, 31),
            (PUSH32, op_push32, 32),
        );
        dispatch_table[JUMP as usize] = Self::op_jump;
        dispatch_table[JUMPI as usize] = Self::op_jumpi;
        dispatch_table[JUMPDEST as usize] = Self::op_jumpdest;
        dispatch_table[MLOAD as usize] = Self::op_mload;
        dispatch_table[MSTORE as usize] = Self::op_mstore;
        dispatch_table[MSTORE8 as usize] = Self::op_mstore8;
        dispatch_table[MSIZE as usize] = Self::op_msize;
        dispatch_table[LT as usize] = Self::op_lt;
        dispatch_table[GT as usize] = Self::op_gt;
        dispatch_table[EQ as usize] = Self::op_eq;
        dispatch_table[ISZERO as usize] = Self::op_iszero;
        dispatch_table[AND as usize] = Self::op_and;
        dispatch_table[OR as usize] = Self::op_or;
        dispatch_table[XOR as usize] = Self::op_xor;
        dispatch_table[NOT as usize] = Self::op_not;
        dispatch_table[BYTE as usize] = Self::op_byte;
        dispatch_table[SHL as usize] = Self::op_shl;
        dispatch_table[SHR as usize] = Self::op_shr;
        dispatch_table[SAR as usize] = Self::op_sar;
        dispatch_table[SLOAD as usize] = Self::op_sload;
        dispatch_table[SSTORE as usize] = Self::op_sstore;
        dispatch_table[TLOAD as usize] = Self::op_tload;
        dispatch_table[TSTORE as usize] = Self::op_tstore;
        dispatch_table[RETURN as usize] = Self::op_return;
        dispatch_table[REVERT as usize] = Self::op_revert;

        // Register Context Opcodes
        dispatch_table[CALLDATALOAD as usize] = Self::op_calldataload;
        dispatch_table[CALLDATASIZE as usize] = Self::op_calldatasize;
        dispatch_table[CALLDATACOPY as usize] = Self::op_calldatacopy;
        dispatch_table[CODECOPY as usize] = Self::op_codecopy;

        dispatch_table[ADDRESS as usize] = Self::op_address;
        dispatch_table[BALANCE as usize] = Self::op_balance;
        dispatch_table[BLOCKHASH as usize] = Self::op_blockhash;
        dispatch_table[CALLER as usize] = Self::op_caller;
        dispatch_table[CALLVALUE as usize] = Self::op_callvalue;
        dispatch_table[CODESIZE as usize] = Self::op_codesize;
        dispatch_table[EXTCODESIZE as usize] = Self::op_extcodesize;
        dispatch_table[EXTCODECOPY as usize] = Self::op_extcodecopy;
        dispatch_table[EXTCODEHASH as usize] = Self::op_extcodehash;
        dispatch_table[SELFBALANCE as usize] = Self::op_selfbalance;
        dispatch_table[GASPRICE as usize] = Self::op_gasprice;
        dispatch_table[RETURNDATASIZE as usize] = Self::op_returndatasize;
        dispatch_table[RETURNDATACOPY as usize] = Self::op_returndatacopy;
        dispatch_table[ORIGIN as usize] = Self::op_origin;
        dispatch_table[COINBASE as usize] = Self::op_coinbase;
        dispatch_table[TIMESTAMP as usize] = Self::op_timestamp;
        dispatch_table[NUMBER as usize] = Self::op_number;
        dispatch_table[PREVRANDAO as usize] = Self::op_prevrandao;
        dispatch_table[GASLIMIT as usize] = Self::op_gaslimit;
        dispatch_table[CHAINID as usize] = Self::op_chainid;
        dispatch_table[BASEFEE as usize] = Self::op_basefee;
        dispatch_table[BLOBHASH as usize] = Self::op_blobhash;
        dispatch_table[BLOBBASEFEE as usize] = Self::op_blobbasefee;
        dispatch_table[GAS as usize] = Self::op_gas;
        dispatch_table[MCOPY as usize] = Self::op_mcopy;
        dispatch_table[LOG0 as usize] = Self::op_log0;
        dispatch_table[LOG1 as usize] = Self::op_log1;
        dispatch_table[LOG2 as usize] = Self::op_log2;
        dispatch_table[LOG3 as usize] = Self::op_log3;
        dispatch_table[LOG4 as usize] = Self::op_log4;
        dispatch_table[CALL as usize] = Self::op_call;
        dispatch_table[CREATE as usize] = Self::op_create;
        dispatch_table[DELEGATECALL as usize] = Self::op_delegatecall;
        dispatch_table[CREATE2 as usize] = Self::op_create2;
        dispatch_table[STATICCALL as usize] = Self::op_staticcall;
        dispatch_table[INVALID as usize] = Self::op_invalid_opcode;
        dispatch_table[SELFDESTRUCT as usize] = Self::op_selfdestruct;

        register_ops!(
            dispatch_table,
            DUP1 => op_dup1,   DUP2 => op_dup2,   DUP3 => op_dup3,   DUP4 => op_dup4,
            DUP5 => op_dup5,   DUP6 => op_dup6,   DUP7 => op_dup7,   DUP8 => op_dup8,
            DUP9 => op_dup9,   DUP10 => op_dup10, DUP11 => op_dup11, DUP12 => op_dup12,
            DUP13 => op_dup13, DUP14 => op_dup14, DUP15 => op_dup15, DUP16 => op_dup16,

            SWAP1 => op_swap1,   SWAP2 => op_swap2,   SWAP3 => op_swap3,   SWAP4 => op_swap4,
            SWAP5 => op_swap5,   SWAP6 => op_swap6,   SWAP7 => op_swap7,   SWAP8 => op_swap8,
            SWAP9 => op_swap9,   SWAP10 => op_swap10, SWAP11 => op_swap11, SWAP12 => op_swap12,
            SWAP13 => op_swap13, SWAP14 => op_swap14, SWAP15 => op_swap15, SWAP16 => op_swap16,
        );

        Self {
            code: code.to_vec(),
            pc: 0,
            stack: Stack::new(),
            memory: Vec::new(),
            storage: HashMap::new(),
            balances: HashMap::new(),
            nonces: HashMap::new(),
            contracts: HashMap::new(),
            accessed_addresses: HashSet::new(),
            accessed_slots: HashSet::new(),
            context: TxContext::default(),
            storage_address: [0; 20],
            transient_storage_address: None,
            env: Environment::default(),
            return_data: Vec::new(),
            logs: Vec::new(),
            is_static: false,
            valid_jumpdests,
            gas_left: gas_limit,
            tracer: None,
            state: StateFork::default(),
            created_in_transaction: HashSet::new(),
            dispatch_table,
            gas_table: build_gas_table(),
        }
    }

    /// Returns `true` if key was already warm, `false` if it was cold (and marks it warm).
    #[inline]
    pub fn mark_slot_warm(&mut self, key: U256) -> bool {
        !self.accessed_slots.insert(key)
    }

    pub fn compute_storage_root(&self, _account: &Address) -> [u8; 32] {
        let mut trie = MerklePatriciaTrie::new();
        for (slot, value) in &self.storage {
            let key = crate::crypto::keccak256(&slot.to_be_bytes::<32>());
            let raw = value.to_be_bytes::<32>();
            let first = raw.iter().position(|byte| *byte != 0).unwrap_or(32);
            let value_bytes = &raw[first..];
            let encoded = if value_bytes.len() == 1 && value_bytes[0] < 0x80 {
                value_bytes.to_vec()
            } else {
                let mut encoded = vec![0x80 + value_bytes.len() as u8];
                encoded.extend_from_slice(value_bytes);
                encoded
            };
            trie.insert(&key, encoded);
        }
        trie.root_hash()
    }

    pub fn compute_state_root(&self) -> [u8; 32] {
        let mut addresses = self.contracts.keys().copied().collect::<HashSet<_>>();
        addresses.extend(self.balances.keys().copied());
        addresses.extend(self.nonces.keys().copied());
        let storage_address = if self.storage_address == [0; 20] {
            self.context.address
        } else {
            self.storage_address
        };
        if !self.storage.is_empty() {
            addresses.insert(storage_address);
        }

        let empty_code_hash = crate::crypto::keccak256(&[]);
        let mut trie = MerklePatriciaTrie::new();
        for address in addresses {
            let code_hash = self
                .contracts
                .get(&address)
                .map(|code| crate::crypto::keccak256(code))
                .unwrap_or(empty_code_hash);
            let account = StateAccount {
                nonce: self.nonces.get(&address).copied().unwrap_or(0),
                balance: self.balances.get(&address).copied().unwrap_or(U256::ZERO),
                storage_root: self.compute_storage_root(&address),
                code_hash,
            };
            let key = crate::crypto::keccak256(&address);
            trie.insert(&key, account.rlp_encode());
        }
        trie.root_hash()
    }

    fn analyze_jumpdests(code: &[u8]) -> Vec<bool> {
        let mut valid = vec![false; code.len()];
        let mut i = 0;
        while i < code.len() {
            let op = code[i];
            if op == JUMPDEST {
                valid[i] = true;
            } else if (PUSH1..=0x7F).contains(&op) {
                let push_len = (op - PUSH1 + 1) as usize;
                i += push_len; // Skip immediate data
            }
            i += 1;
        }
        valid
    }

    #[inline]
    fn memory_cost(words: u64) -> u64 {
        3 * words + (words * words) / 512
    }

    pub fn touch_memory(&mut self, offset: usize, size: usize) -> ExecutionResult {
        if size == 0 {
            return ExecutionResult::Halt;
        }

        let end_bytes = match offset.checked_add(size) {
            Some(end) => end,
            None => return ExecutionResult::OutOfGas,
        };
        let rounded_end = match end_bytes.checked_add(31) {
            Some(end) => end,
            None => return ExecutionResult::OutOfGas,
        };
        let new_words = rounded_end / 32;
        let current_words = self.memory.len() / 32;

        if new_words > current_words {
            let cost_new = Self::memory_cost(new_words as u64);
            let cost_old = Self::memory_cost(current_words as u64);
            let expansion_cost = cost_new - cost_old;
            if self.gas_left < expansion_cost {
                return ExecutionResult::OutOfGas;
            }
            self.gas_left -= expansion_cost;
            self.memory.resize(new_words * 32, 0);
        }

        ExecutionResult::Halt
    }

    fn memory_expansion_cost(&self, ranges: &[(usize, usize)]) -> Result<u64, VmError> {
        let mut required_end = self.memory.len();
        for (offset, size) in ranges {
            let end = offset.checked_add(*size).ok_or(VmError::Overflow)?;
            required_end = required_end.max(end);
        }
        if required_end == 0 {
            return Ok(0);
        }
        let new_words = required_end.checked_add(31).ok_or(VmError::Overflow)? / 32;
        let current_words = self.memory.len() / 32;
        let new_cost = new_words
            .try_into()
            .ok()
            .and_then(|words: u64| {
                let quadratic = words.checked_mul(words)?.checked_div(512)?;
                words.checked_mul(3)?.checked_add(quadratic)
            })
            .ok_or(VmError::Overflow)?;
        let old_words = current_words as u64;
        let old_cost = old_words
            .checked_mul(3)
            .and_then(|cost| {
                old_words
                    .checked_mul(old_words)
                    .and_then(|quadratic| cost.checked_add(quadratic / 512))
            })
            .ok_or(VmError::Overflow)?;
        new_cost.checked_sub(old_cost).ok_or(VmError::Overflow)
    }

    fn expand_memory_without_charge(&mut self, ranges: &[(usize, usize)]) -> Result<(), VmError> {
        let mut required_end = self.memory.len();
        for (offset, size) in ranges {
            required_end = required_end.max(offset.checked_add(*size).ok_or(VmError::Overflow)?);
        }
        let rounded = required_end.checked_add(31).ok_or(VmError::Overflow)? / 32 * 32;
        if rounded > self.memory.len() {
            self.memory.resize(rounded, 0);
        }
        Ok(())
    }

    fn calc_call_base_gas(
        &mut self,
        target: Address,
        value: U256,
        args_off: U256,
        args_size: U256,
        ret_off: U256,
        ret_size: U256,
    ) -> Result<u64, VmError> {
        let memory_cost = self.memory_expansion_cost(&[
            (
                usize::try_from(args_off).map_err(|_| VmError::Overflow)?,
                usize::try_from(args_size).map_err(|_| VmError::Overflow)?,
            ),
            (
                usize::try_from(ret_off).map_err(|_| VmError::Overflow)?,
                usize::try_from(ret_size).map_err(|_| VmError::Overflow)?,
            ),
        ])?;
        let access_cost = if self.accessed_addresses.insert(target) {
            2_600
        } else {
            100
        };
        let value_cost = if value > U256::ZERO {
            9_000u64
                .checked_add(if self.contracts.contains_key(&target) {
                    0
                } else {
                    25_000
                })
                .ok_or(VmError::Overflow)?
        } else {
            0
        };
        700u64
            .checked_add(access_cost)
            .and_then(|cost| cost.checked_add(value_cost))
            .and_then(|cost| cost.checked_add(memory_cost))
            .ok_or(VmError::Overflow)
    }

    fn calc_static_call_base_gas(
        &mut self,
        target: Address,
        args_off: U256,
        args_size: U256,
        ret_off: U256,
        ret_size: U256,
    ) -> Result<u64, VmError> {
        let normal =
            self.calc_call_base_gas(target, U256::ZERO, args_off, args_size, ret_off, ret_size)?;
        normal.checked_sub(660).ok_or(VmError::Overflow)
    }

    pub fn run(&mut self) -> ExecutionResult {
        self.sync_legacy_state_into_fork();
        let result = self.run_without_sync();
        self.sync_fork_into_legacy_state();
        self.state.clear_transient_storage();
        result
    }

    fn run_child(&mut self) -> ExecutionResult {
        self.sync_legacy_state_into_fork();
        let result = self.run_without_sync();
        self.sync_fork_into_legacy_state();
        result
    }

    fn transient_address(&self) -> Address {
        self.transient_storage_address.unwrap_or_else(|| {
            if self.storage_address == [0; 20] {
                self.context.address
            } else {
                self.storage_address
            }
        })
    }

    fn run_without_sync(&mut self) -> ExecutionResult {
        while self.pc < self.code.len() {
            let res = self.step();
            if res != ExecutionResult::Halt {
                return res;
            }
        }
        ExecutionResult::Halt
    }

    fn sync_legacy_state_into_fork(&mut self) {
        let storage_address = if self.storage_address == [0; 20] {
            self.context.address
        } else {
            self.storage_address
        };
        for (slot, value) in &self.storage {
            self.state.set_storage(storage_address, *slot, *value);
        }
        for (address, balance) in &self.balances {
            self.state.set_balance(*address, *balance);
        }
        for (address, nonce) in &self.nonces {
            self.state.set_nonce(*address, *nonce);
        }
        for (address, code) in &self.contracts {
            self.state.set_code(*address, code.clone());
        }
    }

    fn sync_fork_into_legacy_state(&mut self) {
        for (address, account) in &self.state.dirty_state {
            if !account.storage.is_empty() {
                self.storage = account.storage.clone();
            }
            self.balances.insert(*address, account.balance);
            self.nonces.insert(*address, account.nonce);
            if !account.code.is_empty() {
                self.contracts.insert(*address, account.code.clone());
            }
        }
    }

    pub fn step(&mut self) -> ExecutionResult {
        if self.pc >= self.code.len() {
            return ExecutionResult::Halt;
        }
        let op = self.code[self.pc];
        let gas_cost = self.gas_table[op as usize];
        if self.gas_left < gas_cost {
            return ExecutionResult::OutOfGas;
        }
        if let Some(tracer) = &mut self.tracer {
            let frame = StepFrame {
                pc: self.pc,
                op,
                mnemonic: opcode_mnemonic(op),
                gas_left: self.gas_left,
                gas_cost,
                stack: self.stack.values(),
                memory: self.memory.clone(),
            };
            tracer(&frame);
        }
        self.gas_left -= gas_cost;
        self.pc += 1;
        (self.dispatch_table[op as usize])(self)
    }

    // --- Opcode Handlers ---

    fn execute_push_n(evm: &mut Evm, n: usize) -> ExecutionResult {
        let mut bytes = [0u8; 32];
        if evm.pc < evm.code.len() {
            let available = (evm.code.len() - evm.pc).min(n);
            bytes[32 - n..32 - n + available]
                .copy_from_slice(&evm.code[evm.pc..evm.pc + available]);
        }
        evm.pc = evm.pc.saturating_add(n);
        if evm.stack.push(U256::from_be_bytes(bytes)).is_err() {
            return ExecutionResult::Error("Stack Overflow on PUSH");
        }
        ExecutionResult::Halt
    }

    impl_push_n! {
        op_push3 => 3, op_push4 => 4, op_push5 => 5, op_push6 => 6,
        op_push7 => 7, op_push8 => 8, op_push9 => 9, op_push10 => 10,
        op_push11 => 11, op_push12 => 12, op_push13 => 13, op_push14 => 14,
        op_push15 => 15, op_push16 => 16, op_push17 => 17, op_push18 => 18,
        op_push19 => 19, op_push20 => 20, op_push21 => 21, op_push22 => 22,
        op_push23 => 23, op_push24 => 24, op_push25 => 25, op_push26 => 26,
        op_push27 => 27, op_push28 => 28, op_push29 => 29, op_push30 => 30,
        op_push31 => 31, op_push32 => 32,
    }

    fn op_invalid(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Error("Invalid Opcode")
    }

    fn op_invalid_opcode(evm: &mut Evm) -> ExecutionResult {
        evm.gas_left = 0;
        ExecutionResult::InvalidOpcode(INVALID)
    }

    fn op_stop(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Halt
    }

    fn op_jumpdest(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Halt
    }

    fn op_add(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on ADD"),
        };
        if evm.stack.push(a.wrapping_add(b)).is_err() {
            return ExecutionResult::Error("Stack Overflow on ADD");
        }
        ExecutionResult::Halt
    }

    fn op_mul(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on MUL"),
        };
        if evm.stack.push(a.wrapping_mul(b)).is_err() {
            return ExecutionResult::Error("Stack Overflow on MUL");
        }
        ExecutionResult::Halt
    }

    fn op_div(evm: &mut Evm) -> ExecutionResult {
        let (divisor, dividend) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(divisor), Ok(dividend)) => (divisor, dividend),
            _ => return ExecutionResult::Error("Stack Underflow on DIV"),
        };
        let result = if divisor == U256::ZERO {
            U256::ZERO
        } else {
            dividend / divisor
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on DIV"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_mod(evm: &mut Evm) -> ExecutionResult {
        let (modulus, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(modulus), Ok(value)) => (modulus, value),
            _ => return ExecutionResult::Error("Stack Underflow on MOD"),
        };
        let result = if modulus == U256::ZERO {
            U256::ZERO
        } else {
            value % modulus
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on MOD"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_addmod(evm: &mut Evm) -> ExecutionResult {
        let (modulus, b, a) = match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
            (Ok(modulus), Ok(b), Ok(a)) => (modulus, b, a),
            _ => return ExecutionResult::Error("Stack Underflow on ADDMOD"),
        };
        let result = if modulus == U256::ZERO {
            U256::ZERO
        } else {
            let a = a % modulus;
            let b = b % modulus;
            if a >= modulus - b {
                a - (modulus - b)
            } else {
                a + b
            }
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on ADDMOD"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_mulmod(evm: &mut Evm) -> ExecutionResult {
        let (modulus, b, a) = match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
            (Ok(modulus), Ok(b), Ok(a)) => (modulus, b, a),
            _ => return ExecutionResult::Error("Stack Underflow on MULMOD"),
        };
        let result = if modulus == U256::ZERO {
            U256::ZERO
        } else {
            let mut factor = a % modulus;
            let mut multiplier = b;
            let mut result = U256::ZERO;
            while multiplier != U256::ZERO {
                if (multiplier & U256::ONE) != U256::ZERO {
                    result = if result >= modulus - factor {
                        result - (modulus - factor)
                    } else {
                        result + factor
                    };
                }
                factor = if factor >= modulus - factor {
                    factor - (modulus - factor)
                } else {
                    factor + factor
                };
                multiplier >>= 1;
            }
            result
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on MULMOD"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_exp(evm: &mut Evm) -> ExecutionResult {
        let (mut exponent, base) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(exponent), Ok(base)) => (exponent, base),
            _ => return ExecutionResult::Error("Stack Underflow on EXP"),
        };
        let mut factor = base;
        let mut result = U256::ONE;
        while exponent != U256::ZERO {
            if (exponent & U256::ONE) != U256::ZERO {
                result = result.wrapping_mul(factor);
            }
            factor = factor.wrapping_mul(factor);
            exponent >>= 1;
        }
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on EXP"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_sdiv(evm: &mut Evm) -> ExecutionResult {
        let (divisor, dividend) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(divisor), Ok(dividend)) => (divisor, dividend),
            _ => return ExecutionResult::Error("Stack Underflow on SDIV"),
        };
        let sign = U256::ONE << 255;
        let negative_dividend = (dividend & sign) != U256::ZERO;
        let negative_divisor = (divisor & sign) != U256::ZERO;
        let abs = |value: U256| {
            if (value & sign) != U256::ZERO {
                (!value).wrapping_add(U256::ONE)
            } else {
                value
            }
        };
        let result = if divisor == U256::ZERO {
            U256::ZERO
        } else {
            let quotient = abs(dividend) / abs(divisor);
            if negative_dividend ^ negative_divisor {
                (!quotient).wrapping_add(U256::ONE)
            } else {
                quotient
            }
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SDIV"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_smod(evm: &mut Evm) -> ExecutionResult {
        let (modulus, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(modulus), Ok(value)) => (modulus, value),
            _ => return ExecutionResult::Error("Stack Underflow on SMOD"),
        };
        let sign = U256::ONE << 255;
        let negative = (value & sign) != U256::ZERO;
        let abs = |value: U256| {
            if (value & sign) != U256::ZERO {
                (!value).wrapping_add(U256::ONE)
            } else {
                value
            }
        };
        let result = if modulus == U256::ZERO {
            U256::ZERO
        } else {
            let remainder = abs(value) % abs(modulus);
            if negative {
                (!remainder).wrapping_add(U256::ONE)
            } else {
                remainder
            }
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SMOD"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_signextend(evm: &mut Evm) -> ExecutionResult {
        let (byte, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(byte), Ok(value)) => (byte, value),
            _ => return ExecutionResult::Error("Stack Underflow on SIGNEXTEND"),
        };
        let index = as_usize(byte);
        let result = if index >= 32 {
            value
        } else {
            let bit = index * 8 + 7;
            let mask = (U256::ONE << (bit + 1)) - U256::ONE;
            if ((value >> bit) & U256::ONE) != U256::ZERO {
                value | !mask
            } else {
                value & mask
            }
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SIGNEXTEND"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_sub(evm: &mut Evm) -> ExecutionResult {
        let (subtrahend, minuend) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(subtrahend), Ok(minuend)) => (subtrahend, minuend),
            _ => return ExecutionResult::Error("Stack Underflow on SUB"),
        };
        if evm.stack.push(minuend.wrapping_sub(subtrahend)).is_err() {
            return ExecutionResult::Error("Stack Overflow on SUB");
        }
        ExecutionResult::Halt
    }

    fn op_push1(evm: &mut Evm) -> ExecutionResult {
        if evm.pc >= evm.code.len() {
            return ExecutionResult::Error("Unexpected EOF on PUSH1");
        }
        let val = evm.code[evm.pc] as u64;
        evm.pc += 1;
        if evm.stack.push(U256::from(val)).is_err() {
            return ExecutionResult::Error("Stack Overflow on PUSH1");
        }
        ExecutionResult::Halt
    }

    fn op_push2(evm: &mut Evm) -> ExecutionResult {
        if evm.pc + 1 >= evm.code.len() {
            return ExecutionResult::Error("Unexpected EOF on PUSH2");
        }
        let val = u16::from_be_bytes([evm.code[evm.pc], evm.code[evm.pc + 1]]) as u64;
        evm.pc += 2;
        if evm.stack.push(U256::from(val)).is_err() {
            return ExecutionResult::Error("Stack Overflow on PUSH2");
        }
        ExecutionResult::Halt
    }

    fn op_mstore(evm: &mut Evm) -> ExecutionResult {
        let (offset, val) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(o), Ok(v)) => (as_usize(o), v),
            _ => return ExecutionResult::Error("Stack Underflow on MSTORE"),
        };
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, 32) {
            return result;
        }
        evm.memory[offset..offset + 24].fill(0);
        let bytes = val.to_be_bytes::<32>();
        evm.memory[offset..offset + 32].copy_from_slice(&bytes);
        ExecutionResult::Halt
    }

    fn op_mstore8(evm: &mut Evm) -> ExecutionResult {
        let (offset, val) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(o), Ok(v)) => (as_usize(o), v),
            _ => return ExecutionResult::Error("Stack Underflow on MSTORE8"),
        };
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, 1) {
            return result;
        }
        evm.memory[offset] = val.as_limbs()[0] as u8;
        ExecutionResult::Halt
    }

    fn op_mload(evm: &mut Evm) -> ExecutionResult {
        let offset = match evm.stack.pop() {
            Ok(o) => as_usize(o),
            _ => return ExecutionResult::Error("Stack Underflow on MLOAD"),
        };
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, 32) {
            return result;
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&evm.memory[offset..offset + 32]);
        let val = U256::from_be_bytes(bytes);
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Error("Stack Overflow on MLOAD");
        }
        ExecutionResult::Halt
    }

    fn op_msize(evm: &mut Evm) -> ExecutionResult {
        let size = evm.memory.len() as u64;
        if evm.stack.push(U256::from(size)).is_err() {
            return ExecutionResult::Error("Stack Overflow on MSIZE");
        }
        ExecutionResult::Halt
    }

    fn op_mcopy(evm: &mut Evm) -> ExecutionResult {
        let (dest_word, source_word, size_word) =
            match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
                (Ok(dest), Ok(source), Ok(size)) => (dest, source, size),
                _ => return ExecutionResult::Error("Stack Underflow on MCOPY"),
            };
        let size = match usize::try_from(size_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(error) => return ExecutionResult::VmError(error),
        };
        if size == 0 {
            let total_cost = 3u64.checked_add(copy_cost).ok_or(VmError::Overflow);
            let total_cost = match total_cost {
                Ok(cost) => cost,
                Err(error) => return ExecutionResult::VmError(error),
            };
            if evm.gas_left < total_cost {
                return ExecutionResult::OutOfGas;
            }
            evm.gas_left -= total_cost;
            return ExecutionResult::Halt;
        }
        let dest = match usize::try_from(dest_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let source = match usize::try_from(source_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let memory_cost = match evm.memory_expansion_cost(&[(dest, size), (source, size)]) {
            Ok(cost) => cost,
            Err(error) => return ExecutionResult::VmError(error),
        };
        let total_cost = match 3u64
            .checked_add(copy_cost)
            .and_then(|cost| cost.checked_add(memory_cost))
        {
            Some(cost) => cost,
            None => return ExecutionResult::VmError(VmError::Overflow),
        };
        if evm.gas_left < total_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= total_cost;
        if let Err(error) = evm.expand_memory_without_charge(&[(dest, size), (source, size)]) {
            return ExecutionResult::VmError(error);
        }
        evm.memory.copy_within(source..source + size, dest);
        ExecutionResult::Halt
    }

    fn op_lt(evm: &mut Evm) -> ExecutionResult {
        let (right, left) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(right), Ok(left)) => (right, left),
            _ => return ExecutionResult::Error("Stack Underflow on LT"),
        };
        let res = if left < right { U256::ONE } else { U256::ZERO };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Error("Stack Overflow on LT");
        }
        ExecutionResult::Halt
    }

    fn op_gt(evm: &mut Evm) -> ExecutionResult {
        let (right, left) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(right), Ok(left)) => (right, left),
            _ => return ExecutionResult::Error("Stack Underflow on GT"),
        };
        let res = if left > right { U256::ONE } else { U256::ZERO };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Error("Stack Overflow on GT");
        }
        ExecutionResult::Halt
    }

    fn op_eq(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on EQ"),
        };
        let res = if a == b { U256::ONE } else { U256::ZERO };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Error("Stack Overflow on EQ");
        }
        ExecutionResult::Halt
    }

    fn op_iszero(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.pop() {
            Ok(v) => v,
            _ => return ExecutionResult::Error("Stack Underflow on ISZERO"),
        };
        let res = if val == U256::ZERO {
            U256::ONE
        } else {
            U256::ZERO
        };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Error("Stack Overflow on ISZERO");
        }
        ExecutionResult::Halt
    }

    fn op_and(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on AND"),
        };
        if evm.stack.push(a & b).is_err() {
            return ExecutionResult::Error("Stack Overflow on AND");
        }
        ExecutionResult::Halt
    }

    fn op_or(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on OR"),
        };
        if evm.stack.push(a | b).is_err() {
            return ExecutionResult::Error("Stack Overflow on OR");
        }
        ExecutionResult::Halt
    }

    fn op_xor(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Error("Stack Underflow on XOR"),
        };
        if evm.stack.push(a ^ b).is_err() {
            return ExecutionResult::Error("Stack Overflow on XOR");
        }
        ExecutionResult::Halt
    }

    fn op_not(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.pop() {
            Ok(v) => v,
            _ => return ExecutionResult::Error("Stack Underflow on NOT"),
        };
        if evm.stack.push(!val).is_err() {
            return ExecutionResult::Error("Stack Overflow on NOT");
        }
        ExecutionResult::Halt
    }

    fn op_shl(evm: &mut Evm) -> ExecutionResult {
        let (shift, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(shift), Ok(value)) => (shift, value),
            _ => return ExecutionResult::Error("Stack Underflow on SHL"),
        };
        let result = if shift >= U256::from(256u16) {
            U256::ZERO
        } else {
            value << as_usize(shift)
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SHL"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_shr(evm: &mut Evm) -> ExecutionResult {
        let (shift, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(shift), Ok(value)) => (shift, value),
            _ => return ExecutionResult::Error("Stack Underflow on SHR"),
        };
        let result = if shift >= U256::from(256u16) {
            U256::ZERO
        } else {
            value >> as_usize(shift)
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SHR"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_sar(evm: &mut Evm) -> ExecutionResult {
        let (shift, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(shift), Ok(value)) => (shift, value),
            _ => return ExecutionResult::Error("Stack Underflow on SAR"),
        };
        let sign = U256::ONE << 255;
        let negative = (value & sign) != U256::ZERO;
        let result = if shift >= U256::from(256u16) {
            if negative { U256::MAX } else { U256::ZERO }
        } else if shift == U256::ZERO {
            value
        } else {
            let amount = as_usize(shift);
            let shifted = value >> amount;
            if negative {
                shifted | (U256::MAX << (256 - amount))
            } else {
                shifted
            }
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SAR"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_byte(evm: &mut Evm) -> ExecutionResult {
        let (index, value) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(index), Ok(value)) => (index, value),
            _ => return ExecutionResult::Error("Stack Underflow on BYTE"),
        };
        let result = if index >= U256::from(32u8) {
            U256::ZERO
        } else {
            let shift = (31 - as_usize(index)) * 8;
            (value >> shift) & U256::from(0xffu16)
        };
        evm.stack.push(result).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on BYTE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_keccak256(evm: &mut Evm) -> ExecutionResult {
        let (offset_word, size_word) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(offset), Ok(size)) => (offset, size),
            _ => return ExecutionResult::Error("Stack Underflow on KECCAK256"),
        };
        let offset = as_usize(offset_word);
        let size = as_usize(size_word);
        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(_) => return ExecutionResult::OutOfGas,
        };
        if evm.gas_left < copy_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= copy_cost;
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, size) {
            return result;
        }
        let mut hash = [0u8; 32];
        let mut keccak = Keccak::v256();
        keccak.update(&evm.memory[offset..offset + size]);
        keccak.finalize(&mut hash);
        evm.stack.push(U256::from_be_bytes(hash)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on KECCAK256"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_sload(evm: &mut Evm) -> ExecutionResult {
        let key = match evm.stack.pop() {
            Ok(k) => k,
            Err(e) => return ExecutionResult::Error(e),
        };

        let gas_cost = if evm.mark_slot_warm(key) { 100 } else { 2_100 };
        if evm.gas_left < gas_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= gas_cost;

        let storage_address = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        let val = evm.state.get_storage(&storage_address, key);
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Error("Stack Overflow on SLOAD");
        }
        ExecutionResult::Halt
    }

    fn op_sstore(evm: &mut Evm) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let (key, val) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(k), Ok(v)) => (k, v),
            _ => return ExecutionResult::Error("Stack Underflow on SSTORE"),
        };

        let cold_surcharge = if evm.mark_slot_warm(key) { 0 } else { 2_100 };
        let storage_address = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        let current_val = evm.state.get_storage(&storage_address, key);
        let base_cost = if current_val == val {
            100
        } else if current_val == 0 {
            20_000
        } else {
            2_900
        };
        let total_cost = base_cost + cold_surcharge;
        if evm.gas_left < total_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= total_cost;

        evm.state.set_storage(storage_address, key, val);
        ExecutionResult::Halt
    }

    fn op_tload(evm: &mut Evm) -> ExecutionResult {
        let key = match evm.stack.pop() {
            Ok(key) => key,
            Err(error) => return ExecutionResult::Error(error),
        };
        let address = evm.transient_address();
        evm.stack.push(evm.state.tload(&address, &key)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on TLOAD"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_tstore(evm: &mut Evm) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let key = match evm.stack.pop() {
            Ok(key) => key,
            Err(error) => return ExecutionResult::Error(error),
        };
        let value = match evm.stack.pop() {
            Ok(value) => value,
            Err(error) => return ExecutionResult::Error(error),
        };
        let address = evm.transient_address();
        evm.state.tstore(address, key, value);
        ExecutionResult::Halt
    }

    fn op_return(evm: &mut Evm) -> ExecutionResult {
        let offset = match evm.stack.pop() {
            Ok(o) => as_usize(o),
            Err(_) => return ExecutionResult::StackUnderflow,
        };
        let size = match evm.stack.pop() {
            Ok(s) => as_usize(s),
            Err(_) => return ExecutionResult::StackUnderflow,
        };

        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, size) {
            return result;
        }
        let data = evm.memory
            [offset.min(evm.memory.len())..offset.saturating_add(size).min(evm.memory.len())]
            .to_vec();
        ExecutionResult::Return(data)
    }

    fn op_revert(evm: &mut Evm) -> ExecutionResult {
        let offset = match evm.stack.pop() {
            Ok(o) => as_usize(o),
            Err(_) => return ExecutionResult::StackUnderflow,
        };
        let size = match evm.stack.pop() {
            Ok(s) => as_usize(s),
            Err(_) => return ExecutionResult::StackUnderflow,
        };

        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, size) {
            return result;
        }
        let data = evm.memory
            [offset.min(evm.memory.len())..offset.saturating_add(size).min(evm.memory.len())]
            .to_vec();
        ExecutionResult::Revert(data)
    }

    fn op_calldataload(evm: &mut Evm) -> ExecutionResult {
        let offset = match evm.stack.pop() {
            Ok(o) => as_usize(o),
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALLDATALOAD"),
        };

        let mut buf = [0u8; 32];
        let cd = &evm.context.calldata;

        if offset < cd.len() {
            let available = (cd.len() - offset).min(32);
            buf[..available].copy_from_slice(&cd[offset..offset + available]);
        }

        // Slice the high 8 bytes (MSB) instead of trailing zeroes (buf[24..32])
        let val = U256::from_be_bytes(buf);
        if let Err(e) = evm.stack.push(val) {
            return ExecutionResult::Error(e);
        }
        ExecutionResult::Halt
    }

    fn op_calldatasize(evm: &mut Evm) -> ExecutionResult {
        let size = evm.context.calldata.len() as u64;
        if let Err(e) = evm.stack.push(U256::from(size)) {
            return ExecutionResult::Error(e);
        }
        ExecutionResult::Halt
    }

    fn op_calldatacopy(evm: &mut Evm) -> ExecutionResult {
        let (dest, offset, size_word) = match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
            (Ok(d), Ok(o), Ok(s)) => (d, as_usize(o), s),
            _ => return ExecutionResult::Error("Stack Underflow on CALLDATACOPY"),
        };
        let dest_offset = as_usize(dest);
        let size = as_usize(size_word);

        if size == 0 {
            return ExecutionResult::Halt;
        }

        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(_) => return ExecutionResult::OutOfGas,
        };
        if evm.gas_left < copy_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= copy_cost;

        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(dest_offset, size) {
            return result;
        }

        let cd = &evm.context.calldata;
        for i in 0..size {
            let src_idx = offset.saturating_add(i);
            evm.memory[dest_offset + i] = if src_idx < cd.len() { cd[src_idx] } else { 0 };
        }

        ExecutionResult::Halt
    }

    fn op_codecopy(evm: &mut Evm) -> ExecutionResult {
        let (dest, code_offset, size_word) =
            match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
                (Ok(d), Ok(o), Ok(s)) => (d, as_usize(o), s),
                _ => return ExecutionResult::Error("Stack Underflow on CODECOPY"),
            };
        let dest_offset = as_usize(dest);
        let size = as_usize(size_word);
        if size == 0 {
            return ExecutionResult::Halt;
        }
        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(_) => return ExecutionResult::OutOfGas,
        };
        if evm.gas_left < copy_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= copy_cost;
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(dest_offset, size) {
            return result;
        }
        for i in 0..size {
            let source = code_offset.saturating_add(i);
            evm.memory[dest_offset + i] = evm.code.get(source).copied().unwrap_or(0);
        }
        ExecutionResult::Halt
    }

    fn op_address(evm: &mut Evm) -> ExecutionResult {
        let address = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        Self::push_address(evm, address)
    }

    fn op_caller(evm: &mut Evm) -> ExecutionResult {
        Self::push_address(evm, evm.context.caller)
    }

    fn op_callvalue(evm: &mut Evm) -> ExecutionResult {
        let val = evm.context.value as u64;
        if let Err(e) = evm.stack.push(U256::from(val)) {
            return ExecutionResult::Error(e);
        }
        ExecutionResult::Halt
    }

    fn push_address(evm: &mut Evm, address: Address) -> ExecutionResult {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(&address);
        evm.stack.push(U256::from_be_bytes(bytes)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on context opcode"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_codesize(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(U256::from(evm.code.len())).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on CODESIZE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn account_address(word: U256) -> Address {
        let bytes = word.to_be_bytes::<32>();
        let mut address = [0u8; 20];
        address.copy_from_slice(&bytes[12..]);
        address
    }

    fn charge_account_access(evm: &mut Evm, address: Address) -> ExecutionResult {
        let cost = if evm.accessed_addresses.insert(address) {
            2_600
        } else {
            100
        };
        if evm.gas_left < cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= cost;
        ExecutionResult::Halt
    }

    fn account_code(evm: &Evm, address: &Address) -> Vec<u8> {
        evm.state.get_code(address)
    }

    fn op_balance(evm: &mut Evm) -> ExecutionResult {
        let address = match evm.stack.pop() {
            Ok(word) => Self::account_address(word),
            Err(error) => return ExecutionResult::Error(error),
        };
        if let result @ ExecutionResult::OutOfGas = Self::charge_account_access(evm, address) {
            return result;
        }
        evm.stack.push(evm.state.get_balance(&address)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on BALANCE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_selfbalance(evm: &mut Evm) -> ExecutionResult {
        let address = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        evm.stack.push(evm.state.get_balance(&address)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on SELFBALANCE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_blockhash(evm: &mut Evm) -> ExecutionResult {
        let requested = match evm.stack.pop() {
            Ok(value) => value,
            Err(error) => return ExecutionResult::Error(error),
        };
        let current = match u64::try_from(evm.env.number) {
            Ok(value) => value,
            Err(_) => return Self::push_u256(evm, U256::ZERO, "BLOCKHASH"),
        };
        let hash = u64::try_from(requested)
            .ok()
            .filter(|number| *number < current && current - *number <= 256)
            .and_then(|number| {
                evm.env
                    .block_hashes
                    .get(&number)
                    .copied()
                    .or_else(|| Some((evm.env.block_hash)(number)))
            })
            .unwrap_or(U256::ZERO);
        Self::push_u256(evm, hash, "BLOCKHASH")
    }

    fn push_u256(evm: &mut Evm, value: U256, opcode: &str) -> ExecutionResult {
        evm.stack.push(value).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on block context opcode"),
            |_| {
                let _ = opcode;
                ExecutionResult::Halt
            },
        )
    }

    fn op_extcodesize(evm: &mut Evm) -> ExecutionResult {
        let address = match evm.stack.pop() {
            Ok(word) => Self::account_address(word),
            Err(error) => return ExecutionResult::Error(error),
        };
        if let result @ ExecutionResult::OutOfGas = Self::charge_account_access(evm, address) {
            return result;
        }
        evm.stack
            .push(U256::from(Self::account_code(evm, &address).len()))
            .map_or_else(
                |_| ExecutionResult::Error("Stack Overflow on EXTCODESIZE"),
                |_| ExecutionResult::Halt,
            )
    }

    fn op_extcodehash(evm: &mut Evm) -> ExecutionResult {
        let address = match evm.stack.pop() {
            Ok(word) => Self::account_address(word),
            Err(error) => return ExecutionResult::Error(error),
        };
        if let result @ ExecutionResult::OutOfGas = Self::charge_account_access(evm, address) {
            return result;
        }
        let code = Self::account_code(evm, &address);
        let hash = if code.is_empty() {
            [0u8; 32]
        } else {
            crate::crypto::keccak256(&code)
        };
        evm.stack.push(U256::from_be_bytes(hash)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on EXTCODEHASH"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_extcodecopy(evm: &mut Evm) -> ExecutionResult {
        let address = match evm.stack.pop() {
            Ok(word) => Self::account_address(word),
            Err(error) => return ExecutionResult::Error(error),
        };
        let (dest_word, offset_word, size_word) =
            match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
                (Ok(dest), Ok(offset), Ok(size)) => (dest, offset, size),
                _ => return ExecutionResult::Error("Stack Underflow on EXTCODECOPY"),
            };
        let dest = match usize::try_from(dest_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let offset = match usize::try_from(offset_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let size = match usize::try_from(size_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(_) => return ExecutionResult::OutOfGas,
        };
        let access_cost: u64 = if evm.accessed_addresses.insert(address) {
            2_600
        } else {
            100
        };
        let memory_cost = match evm.memory_expansion_cost(&[(dest, size)]) {
            Ok(cost) => cost,
            Err(error) => return ExecutionResult::VmError(error),
        };
        let total_cost = match access_cost
            .checked_add(copy_cost)
            .and_then(|cost| cost.checked_add(memory_cost))
        {
            Some(cost) => cost,
            None => return ExecutionResult::VmError(VmError::Overflow),
        };
        if evm.gas_left < total_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= total_cost;
        if let Err(error) = evm.expand_memory_without_charge(&[(dest, size)]) {
            return ExecutionResult::VmError(error);
        }
        let code = Self::account_code(evm, &address);
        for index in 0..size {
            let source = offset.saturating_add(index);
            evm.memory[dest + index] = code.get(source).copied().unwrap_or(0);
        }
        ExecutionResult::Halt
    }

    fn op_gasprice(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.gas_price).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on GASPRICE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_origin(evm: &mut Evm) -> ExecutionResult {
        Self::push_address(evm, evm.env.origin)
    }
    fn op_coinbase(evm: &mut Evm) -> ExecutionResult {
        Self::push_address(evm, evm.env.coinbase)
    }

    fn op_timestamp(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.timestamp).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on TIMESTAMP"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_number(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.number).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on NUMBER"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_prevrandao(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.prevrandao).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on PREVRANDAO"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_gaslimit(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.gas_limit).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on GASLIMIT"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_chainid(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.chain_id).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on CHAINID"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_basefee(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.base_fee).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on BASEFEE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_blobhash(evm: &mut Evm) -> ExecutionResult {
        let index = match evm.stack.pop() {
            Ok(index) => index,
            Err(error) => return ExecutionResult::Error(error),
        };
        let value = usize::try_from(index)
            .ok()
            .and_then(|index| evm.context.blob_versioned_hashes.get(index))
            .copied()
            .map(U256::from_be_bytes)
            .unwrap_or(U256::ZERO);
        evm.stack.push(value).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on BLOBHASH"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_blobbasefee(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(evm.env.blob_base_fee).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on BLOBBASEFEE"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_gas(evm: &mut Evm) -> ExecutionResult {
        evm.stack.push(U256::from(evm.gas_left)).map_or_else(
            |_| ExecutionResult::Error("Stack Overflow on GAS"),
            |_| ExecutionResult::Halt,
        )
    }

    fn op_returndatasize(evm: &mut Evm) -> ExecutionResult {
        evm.stack
            .push(U256::from(evm.return_data.len()))
            .map_or_else(
                |_| ExecutionResult::Error("Stack Overflow on RETURNDATASIZE"),
                |_| ExecutionResult::Halt,
            )
    }

    fn op_returndatacopy(evm: &mut Evm) -> ExecutionResult {
        let (dest, offset, size_word) = match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
            (Ok(dest), Ok(offset), Ok(size)) => (as_usize(dest), as_usize(offset), size),
            _ => return ExecutionResult::Error("Stack Underflow on RETURNDATACOPY"),
        };
        let size = as_usize(size_word);
        let end = match offset.checked_add(size) {
            Some(end) if end <= evm.return_data.len() => end,
            _ => return ExecutionResult::VmError(VmError::OutOfBoundsReturnData),
        };
        let copy_cost = match calc_copy_gas(size_word) {
            Ok(cost) => cost,
            Err(_) => return ExecutionResult::OutOfGas,
        };
        if evm.gas_left < copy_cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= copy_cost;
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(dest, size) {
            return result;
        }
        evm.memory[dest..dest + size].copy_from_slice(&evm.return_data[offset..end]);
        ExecutionResult::Halt
    }

    fn op_create(evm: &mut Evm) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let (value, offset, size) = match (evm.stack.pop(), evm.stack.pop(), evm.stack.pop()) {
            (Ok(value), Ok(offset), Ok(size)) => (value, offset, size),
            _ => return ExecutionResult::Error("Stack Underflow on CREATE"),
        };
        Self::execute_create(evm, value, offset, size, None)
    }

    fn op_create2(evm: &mut Evm) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let (value, offset, size, salt) = match (
            evm.stack.pop(),
            evm.stack.pop(),
            evm.stack.pop(),
            evm.stack.pop(),
        ) {
            (Ok(value), Ok(offset), Ok(size), Ok(salt)) => (value, offset, size, salt),
            _ => return ExecutionResult::Error("Stack Underflow on CREATE2"),
        };
        Self::execute_create(evm, value, offset, size, Some(salt))
    }

    fn execute_create(
        evm: &mut Evm,
        value: U256,
        offset_word: U256,
        size_word: U256,
        salt: Option<U256>,
    ) -> ExecutionResult {
        let offset = match usize::try_from(offset_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let size = match usize::try_from(size_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let expansion = match evm.memory_expansion_cost(&[(offset, size)]) {
            Ok(cost) => cost,
            Err(error) => return ExecutionResult::VmError(error),
        };
        let hash_gas = match salt {
            Some(_) => match calc_copy_gas(size_word) {
                Ok(words) => match words.checked_mul(2) {
                    Some(cost) => cost,
                    None => return ExecutionResult::VmError(VmError::Overflow),
                },
                Err(error) => return ExecutionResult::VmError(error),
            },
            None => 0,
        };
        let base_gas = match 32_000u64
            .checked_add(expansion)
            .and_then(|cost| cost.checked_add(hash_gas))
        {
            Some(cost) => cost,
            None => return ExecutionResult::VmError(VmError::Overflow),
        };
        if evm.gas_left < base_gas {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= base_gas;
        if let Err(error) = evm.expand_memory_without_charge(&[(offset, size)]) {
            return ExecutionResult::VmError(error);
        }
        let init_code = evm.memory[offset..offset + size].to_vec();
        let sender = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        let nonce = evm.nonces.get(&sender).copied().unwrap_or(0);
        evm.nonces.insert(sender, nonce.saturating_add(1));
        let new_addr = salt.map_or_else(
            || derive_create_address(&sender, nonce),
            |salt| derive_create2_address(&sender, salt, &init_code),
        );
        let child_gas = calc_eip150_call_gas(evm.gas_left, U256::from(evm.gas_left));
        evm.gas_left -= child_gas;
        match evm.execute_deploy(new_addr, value, &init_code, child_gas) {
            Ok(address) => evm.stack.push(address).map_or_else(
                |_| ExecutionResult::Error("Stack Overflow on CREATE"),
                |_| ExecutionResult::Halt,
            ),
            Err(VmError::StaticCallViolation) => {
                ExecutionResult::VmError(VmError::StaticCallViolation)
            }
            Err(_) => evm.stack.push(U256::ZERO).map_or_else(
                |_| ExecutionResult::Error("Stack Overflow on CREATE"),
                |_| ExecutionResult::Halt,
            ),
        }
    }

    pub fn execute_deploy(
        &mut self,
        new_addr: Address,
        value: U256,
        init_code: &[u8],
        gas_limit: u64,
    ) -> Result<U256, VmError> {
        if self.is_static {
            return Err(VmError::StaticCallViolation);
        }
        let sender = if self.storage_address == [0; 20] {
            self.context.address
        } else {
            self.storage_address
        };
        let sender_balance = self.balances.get(&sender).copied().unwrap_or(U256::ZERO);
        if sender_balance < value {
            return Ok(U256::ZERO);
        }
        let snapshot = self.state.snapshot();
        self.balances.insert(sender, sender_balance - value);
        let target_balance = self.balances.get(&new_addr).copied().unwrap_or(U256::ZERO);
        self.balances.insert(new_addr, target_balance + value);
        self.state.set_balance(sender, sender_balance - value);
        self.state.set_balance(new_addr, target_balance + value);

        let mut child = Evm::new_with_gas(init_code, gas_limit);
        child.env = self.env.clone();
        child.context.caller = self.context.address;
        child.context.address = new_addr;
        child.context.value = u128::try_from(value).unwrap_or(u128::MAX);
        child.storage_address = new_addr;
        child.transient_storage_address = Some(new_addr);
        child.state = self.state.clone();
        child.storage = self.storage.clone();
        child.balances = self.balances.clone();
        child.nonces = self.nonces.clone();
        child.contracts = self.contracts.clone();
        child.created_in_transaction = self.created_in_transaction.clone();
        child.is_static = false;

        let result = child.run_child();
        self.gas_left = self.gas_left.saturating_add(child.gas_left);
        self.created_in_transaction
            .extend(child.created_in_transaction.iter().copied());
        let runtime_code = match result {
            ExecutionResult::Return(code) => {
                child.return_data = code.clone();
                self.return_data = code.clone();
                code
            }
            ExecutionResult::Halt => Vec::new(),
            _ => {
                self.return_data = match result {
                    ExecutionResult::Revert(data) => data,
                    _ => Vec::new(),
                };
                self.balances.insert(sender, sender_balance);
                self.balances.insert(new_addr, target_balance);
                self.state.revert_to_snapshot(snapshot);
                return Ok(U256::ZERO);
            }
        };
        if runtime_code.len() > 24_576 {
            self.balances.insert(sender, sender_balance);
            self.balances.insert(new_addr, target_balance);
            self.state.revert_to_snapshot(snapshot);
            return Ok(U256::ZERO);
        }
        let deposit = match u64::try_from(runtime_code.len())
            .ok()
            .and_then(|length| length.checked_mul(200))
        {
            Some(cost) => cost,
            None => {
                self.balances.insert(sender, sender_balance);
                self.balances.insert(new_addr, target_balance);
                self.state.revert_to_snapshot(snapshot);
                return Err(VmError::Overflow);
            }
        };
        if self.gas_left < deposit {
            self.balances.insert(sender, sender_balance);
            self.balances.insert(new_addr, target_balance);
            self.state.revert_to_snapshot(snapshot);
            return Ok(U256::ZERO);
        }
        self.gas_left -= deposit;
        self.contracts.insert(new_addr, runtime_code);
        self.created_in_transaction.insert(new_addr);
        self.state = child.state;
        self.state
            .set_code(new_addr, self.contracts[&new_addr].clone());
        self.storage = child.storage;
        self.balances = child.balances;
        self.nonces = child.nonces;
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(&new_addr);
        Ok(U256::from_be_bytes(bytes))
    }

    fn op_call(evm: &mut Evm) -> ExecutionResult {
        Self::execute_call(evm, false, false)
    }

    fn op_delegatecall(evm: &mut Evm) -> ExecutionResult {
        Self::execute_call(evm, false, true)
    }

    fn op_staticcall(evm: &mut Evm) -> ExecutionResult {
        Self::execute_call(evm, true, false)
    }

    fn op_selfdestruct(evm: &mut Evm) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let beneficiary = match evm.stack.pop() {
            Ok(value) => Self::account_address(value),
            Err(error) => return ExecutionResult::Error(error),
        };
        let access_cost: u64 = if evm.accessed_addresses.insert(beneficiary) {
            2_600
        } else {
            100
        };
        let cost = 5_000 + access_cost;
        if evm.gas_left < cost {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= cost;

        let account = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        let balance = evm.state.get_balance(&account);
        if account != beneficiary {
            let beneficiary_balance = evm.state.get_balance(&beneficiary);
            evm.state
                .set_balance(beneficiary, beneficiary_balance + balance);
            evm.state.set_balance(account, U256::ZERO);
        }
        if evm.created_in_transaction.contains(&account) {
            evm.state.dirty_state.remove(&account);
            evm.state.base_state.remove(&account);
            evm.contracts.remove(&account);
            evm.balances.remove(&account);
            evm.nonces.remove(&account);
            if evm.storage_address == account {
                evm.storage.clear();
            }
        }
        ExecutionResult::Halt
    }

    fn execute_call(evm: &mut Evm, force_static: bool, delegate: bool) -> ExecutionResult {
        let gas_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };
        let address_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };
        let value = if force_static || delegate {
            U256::ZERO
        } else {
            match evm.stack.pop() {
                Ok(value) => value,
                Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
            }
        };
        let args_offset_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };
        let args_size_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };
        let ret_offset_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };
        let ret_size_word = match evm.stack.pop() {
            Ok(value) => value,
            Err(_) => return ExecutionResult::Error("Stack Underflow on CALL"),
        };

        let args_offset = match usize::try_from(args_offset_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let args_size = match usize::try_from(args_size_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let ret_offset = match usize::try_from(ret_offset_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let ret_size = match usize::try_from(ret_size_word) {
            Ok(value) => value,
            Err(_) => return ExecutionResult::VmError(VmError::Overflow),
        };
        let mut target = [0u8; 20];
        let address_bytes = address_word.to_be_bytes::<32>();
        target.copy_from_slice(&address_bytes[12..]);
        let base_value = if force_static || delegate {
            U256::ZERO
        } else {
            value
        };
        let base_gas_result = if force_static {
            evm.calc_static_call_base_gas(
                target,
                args_offset_word,
                args_size_word,
                ret_offset_word,
                ret_size_word,
            )
        } else {
            evm.calc_call_base_gas(
                target,
                base_value,
                args_offset_word,
                args_size_word,
                ret_offset_word,
                ret_size_word,
            )
        };
        let base_gas = match base_gas_result {
            Ok(cost) => cost,
            Err(error) => return ExecutionResult::VmError(error),
        };
        if evm.gas_left < base_gas {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= base_gas;
        if let Err(error) =
            evm.expand_memory_without_charge(&[(args_offset, args_size), (ret_offset, ret_size)])
        {
            return ExecutionResult::VmError(error);
        }
        let input = evm.memory[args_offset..args_offset + args_size].to_vec();
        let call_gas = calc_eip150_call_gas(evm.gas_left, gas_word);
        evm.gas_left -= call_gas;

        if is_precompile(&target) {
            let (gas_used, output) = match execute_precompile(&target, &input, call_gas) {
                Ok(result) => result,
                Err(VmError::OutOfGas) => return ExecutionResult::OutOfGas,
                Err(error) => return ExecutionResult::VmError(error),
            };
            evm.gas_left = evm.gas_left.saturating_add(call_gas - gas_used);
            evm.return_data = output.clone();
            evm.memory[ret_offset..ret_offset + ret_size].fill(0);
            let copied = output.len().min(ret_size);
            evm.memory[ret_offset..ret_offset + copied].copy_from_slice(&output[..copied]);
            if evm.stack.push(U256::ONE).is_err() {
                return ExecutionResult::Error("Stack Overflow on CALL");
            }
            return ExecutionResult::Halt;
        }

        let frame = CallFrame {
            gas_limit: call_gas,
            target,
            is_static: evm.is_static || force_static,
            value,
            input: input.clone(),
        };
        let call_context = CallContext {
            caller: if delegate {
                evm.context.caller
            } else {
                evm.context.address
            },
            code_address: frame.target,
            storage_address: if delegate {
                if evm.storage_address == [0; 20] {
                    evm.context.address
                } else {
                    evm.storage_address
                }
            } else {
                frame.target
            },
            value: if delegate {
                U256::from(evm.context.value)
            } else {
                frame.value
            },
            is_static: frame.is_static,
        };
        let child_code = evm.state.get_code(&frame.target);
        let snapshot = evm.state.snapshot();
        let mut child = Evm::new_with_gas(&child_code, frame.gas_limit);
        child.env = evm.env.clone();
        child.context.caller = call_context.caller;
        child.context.address = frame.target;
        child.storage_address = call_context.storage_address;
        child.transient_storage_address = Some(call_context.storage_address);
        child.context.value = u128::try_from(call_context.value).unwrap_or(u128::MAX);
        child.context.calldata = frame.input;
        child.state = evm.state.clone();
        child.storage = evm.storage.clone();
        child.contracts = evm.contracts.clone();
        child.created_in_transaction = evm.created_in_transaction.clone();
        child.is_static = frame.is_static;

        let result = child.run_child();
        evm.gas_left = evm.gas_left.saturating_add(child.gas_left);
        let (success, output) = match result {
            ExecutionResult::Halt => (true, Vec::new()),
            ExecutionResult::Return(data) => (true, data),
            ExecutionResult::Revert(data) => (false, data),
            _ => (false, Vec::new()),
        };
        evm.return_data = output.clone();
        if success {
            evm.state = child.state;
            evm.storage = child.storage;
            evm.created_in_transaction = child.created_in_transaction;
            evm.logs.extend(child.logs);
        } else {
            evm.state.revert_to_snapshot(snapshot);
        }
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(ret_offset, ret_size) {
            return result;
        }
        evm.memory[ret_offset..ret_offset + ret_size].fill(0);
        let copied = output.len().min(ret_size);
        evm.memory[ret_offset..ret_offset + copied].copy_from_slice(&output[..copied]);
        if evm
            .stack
            .push(if success { U256::ONE } else { U256::ZERO })
            .is_err()
        {
            return ExecutionResult::Error("Stack Overflow on CALL");
        }
        ExecutionResult::Halt
    }

    fn op_log(evm: &mut Evm, topic_count: usize) -> ExecutionResult {
        if evm.is_static {
            return ExecutionResult::VmError(VmError::StaticCallViolation);
        }
        let (offset, size_word) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(offset), Ok(size)) => (as_usize(offset), size),
            _ => return ExecutionResult::Error("Stack Underflow on LOG"),
        };
        let size = as_usize(size_word);
        let mut topics = Vec::with_capacity(topic_count);
        for _ in 0..topic_count {
            match evm.stack.pop() {
                Ok(topic) => topics.push(topic),
                Err(_) => return ExecutionResult::Error("Stack Underflow on LOG"),
            }
        }
        let data_gas = match u64::try_from(size)
            .ok()
            .and_then(|size| size.checked_mul(8))
        {
            Some(cost) => cost,
            None => return ExecutionResult::OutOfGas,
        };
        let base_gas = match 375u64
            .checked_add(375u64.saturating_mul(topic_count as u64))
            .and_then(|cost| cost.checked_add(data_gas))
        {
            Some(cost) => cost,
            None => return ExecutionResult::OutOfGas,
        };
        if evm.gas_left < base_gas {
            return ExecutionResult::OutOfGas;
        }
        evm.gas_left -= base_gas;
        if let result @ ExecutionResult::OutOfGas = evm.touch_memory(offset, size) {
            return result;
        }
        let data = evm.memory[offset..offset + size].to_vec();
        let address = if evm.storage_address == [0; 20] {
            evm.context.address
        } else {
            evm.storage_address
        };
        evm.logs.push(LogRecord {
            address,
            topics,
            data,
        });
        ExecutionResult::Halt
    }

    impl_log_n! {
        (op_log0, 0), (op_log1, 1), (op_log2, 2), (op_log3, 3), (op_log4, 4),
    }

    fn op_jump(evm: &mut Evm) -> ExecutionResult {
        let dest = match evm.stack.pop() {
            Ok(d) => as_usize(d),
            _ => return ExecutionResult::Error("Stack Underflow on JUMP"),
        };
        if !evm.valid_jumpdests.get(dest).copied().unwrap_or(false) {
            return ExecutionResult::Error("Invalid JUMP destination");
        }
        evm.pc = dest;
        ExecutionResult::Halt
    }

    fn op_jumpi(evm: &mut Evm) -> ExecutionResult {
        let (dest, cond) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(d), Ok(c)) => (as_usize(d), c),
            _ => return ExecutionResult::Error("Stack Underflow on JUMPI"),
        };
        if cond != U256::ZERO {
            if !evm.valid_jumpdests.get(dest).copied().unwrap_or(false) {
                return ExecutionResult::Error("Invalid JUMPI destination");
            }
            evm.pc = dest;
        }
        ExecutionResult::Halt
    }

    impl_dup_swap_handlers! {
        dup: (1, op_dup1),   (2, op_dup2),   (3, op_dup3),   (4, op_dup4),
             (5, op_dup5),   (6, op_dup6),   (7, op_dup7),   (8, op_dup8),
             (9, op_dup9),   (10, op_dup10), (11, op_dup11), (12, op_dup12),
             (13, op_dup13), (14, op_dup14), (15, op_dup15), (16, op_dup16);

        swap: (1, op_swap1),   (2, op_swap2),   (3, op_swap3),   (4, op_swap4),
              (5, op_swap5),   (6, op_swap6),   (7, op_swap7),   (8, op_swap8),
              (9, op_swap9),   (10, op_swap10), (11, op_swap11), (12, op_swap12),
              (13, op_swap13), (14, op_swap14), (15, op_swap15), (16, op_swap16)
    }
}
