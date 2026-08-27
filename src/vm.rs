use crate::opcodes::*;
use crate::stack::Stack;

#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionResult {
    Halt,
    Revert(&'static str),
}

type OpFn = fn(&mut Evm) -> ExecutionResult;

macro_rules! impl_dup_swap_handlers {
    (
        dup: $( ($n:literal, $dup_fn:ident) ),+ ;
        swap: $( ($sn:literal, $swap_fn:ident) ),+
    ) => {
        $(
            fn $dup_fn(evm: &mut Evm) -> ExecutionResult {
                let val = match evm.stack.peek($n) {
                    Ok(v) => v,
                    Err(e) => return ExecutionResult::Revert(e),
                };
                if evm.stack.push(val).is_err() {
                    return ExecutionResult::Revert(concat!("Stack Overflow on DUP", stringify!($n)));
                }
                ExecutionResult::Halt
            }
        )+

        $(
            fn $swap_fn(evm: &mut Evm) -> ExecutionResult {
                if let Err(e) = evm.stack.swap($sn) {
                    return ExecutionResult::Revert(e);
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

pub struct Evm<'a> {
    code: &'a [u8],
    pub pc: usize,
    pub stack: Stack<1024>,
    pub memory: Vec<u8>,
    dispatch_table: [OpFn; 256],
}

impl<'a> Evm<'a> {
    pub fn new(code: &'a [u8]) -> Self {
        let mut dispatch_table: [OpFn; 256] = [Self::op_invalid; 256];

        // Register opcodes
        dispatch_table[STOP as usize] = Self::op_stop;
        dispatch_table[ADD as usize] = Self::op_add;
        dispatch_table[SUB as usize] = Self::op_sub;
        dispatch_table[PUSH1 as usize] = Self::op_push1;
        dispatch_table[JUMP as usize] = Self::op_jump;
        dispatch_table[JUMPI as usize] = Self::op_jumpi;
        dispatch_table[JUMPDEST as usize] = Self::op_jumpdest;
        dispatch_table[MLOAD as usize] = Self::op_mload;
        dispatch_table[MSTORE as usize] = Self::op_mstore;
        dispatch_table[MSIZE as usize] = Self::op_msize;
        //dispatch_table[DUP1 as usize] = Self::op_dup1;
        //dispatch_table[DUP2 as usize] = Self::op_dup2;
        //dispatch_table[SWAP1 as usize] = Self::op_swap1;
        //dispatch_table[SWAP2 as usize] = Self::op_swap2;
        dispatch_table[LT as usize] = Self::op_lt;
        dispatch_table[GT as usize] = Self::op_gt;
        dispatch_table[EQ as usize] = Self::op_eq;
        dispatch_table[ISZERO as usize] = Self::op_iszero;
        dispatch_table[AND as usize] = Self::op_and;
        dispatch_table[OR as usize] = Self::op_or;
        dispatch_table[XOR as usize] = Self::op_xor;
        dispatch_table[NOT as usize] = Self::op_not;

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
            code,
            pc: 0,
            stack: Stack::new(),
            memory: Vec::new(),
            dispatch_table,
        }
    }

    #[inline(always)]
    fn ensure_memory(&mut self, offset: usize, size: usize) {
        let req = offset.saturating_add(size);
        if req > self.memory.len() {
            let rounded = (req + 31) & !31;
            self.memory.resize(rounded, 0);
        }
    }

    pub fn run(&mut self) -> ExecutionResult {
        while self.pc < self.code.len() {
            let op = self.code[self.pc];
            self.pc += 1;

            let res = (self.dispatch_table[op as usize])(self);
            if res != ExecutionResult::Halt && !matches!(res, ExecutionResult::Halt) {
                // If handler returns anything other than a normal step continue
                if res != ExecutionResult::Halt {
                    return res;
                }
            }
        }
        ExecutionResult::Halt
    }

    // --- Opcode Handlers ---

    fn op_invalid(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Revert("Invalid Opcode")
    }

    fn op_stop(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Halt
    }

    fn op_jumpdest(_evm: &mut Evm) -> ExecutionResult {
        ExecutionResult::Halt // Continue loop
    }

    fn op_add(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on ADD"),
        };
        if evm.stack.push(a.wrapping_add(b)).is_err() {
            return ExecutionResult::Revert("Stack Overflow on ADD");
        }
        ExecutionResult::Halt
    }

    fn op_sub(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on SUB"),
        };
        if evm.stack.push(a.wrapping_sub(b)).is_err() {
            return ExecutionResult::Revert("Stack Overflow on SUB");
        }
        ExecutionResult::Halt
    }

    fn op_push1(evm: &mut Evm) -> ExecutionResult {
        if evm.pc >= evm.code.len() {
            return ExecutionResult::Revert("Unexpected EOF on PUSH1");
        }
        let val = evm.code[evm.pc] as u64;
        evm.pc += 1;
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Revert("Stack Overflow on PUSH1");
        }
        ExecutionResult::Halt
    }

    fn op_jump(evm: &mut Evm) -> ExecutionResult {
        let dest = match evm.stack.pop() {
            Ok(d) => d as usize,
            _ => return ExecutionResult::Revert("Stack Underflow on JUMP"),
        };
        if dest >= evm.code.len() || evm.code[dest] != JUMPDEST {
            return ExecutionResult::Revert("Invalid JUMP destination");
        }
        evm.pc = dest;
        ExecutionResult::Halt
    }

    fn op_jumpi(evm: &mut Evm) -> ExecutionResult {
        let (dest, cond) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(d), Ok(c)) => (d as usize, c),
            _ => return ExecutionResult::Revert("Stack Underflow on JUMPI"),
        };
        if cond != 0 {
            if dest >= evm.code.len() || evm.code[dest] != JUMPDEST {
                return ExecutionResult::Revert("Invalid JUMPI destination");
            }
            evm.pc = dest;
        }
        ExecutionResult::Halt
    }

    fn op_mstore(evm: &mut Evm) -> ExecutionResult {
        let (offset, val) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(o), Ok(v)) => (o as usize, v),
            _ => return ExecutionResult::Revert("Stack Underflow on MSTORE"),
        };
        evm.ensure_memory(offset, 32);
        evm.memory[offset..offset + 24].fill(0);
        evm.memory[offset + 24..offset + 32].copy_from_slice(&val.to_be_bytes());
        ExecutionResult::Halt
    }

    fn op_mload(evm: &mut Evm) -> ExecutionResult {
        let offset = match evm.stack.pop() {
            Ok(o) => o as usize,
            _ => return ExecutionResult::Revert("Stack Underflow on MLOAD"),
        };
        evm.ensure_memory(offset, 32);
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&evm.memory[offset + 24..offset + 32]);
        let val = u64::from_be_bytes(bytes);
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Revert("Stack Overflow on MLOAD");
        }
        ExecutionResult::Halt
    }

    fn op_msize(evm: &mut Evm) -> ExecutionResult {
        let size = evm.memory.len() as u64;
        if evm.stack.push(size).is_err() {
            return ExecutionResult::Revert("Stack Overflow on MSIZE");
        }
        ExecutionResult::Halt
    }
/*
    fn op_dup1(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.peek(1) {
            Ok(v) => v,
            Err(e) => return ExecutionResult::Revert(e),
        };
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Revert("Stack Overflow on DUP1");
        }
        ExecutionResult::Halt
    }

    fn op_dup2(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.peek(2) {
            Ok(v) => v,
            Err(e) => return ExecutionResult::Revert(e),
        };
        if evm.stack.push(val).is_err() {
            return ExecutionResult::Revert("Stack Overflow on DUP2");
        }
        ExecutionResult::Halt
    }

    fn op_swap1(evm: &mut Evm) -> ExecutionResult {
        if let Err(e) = evm.stack.swap(1) {
            return ExecutionResult::Revert(e);
        }
        ExecutionResult::Halt
    }

    fn op_swap2(evm: &mut Evm) -> ExecutionResult {
        if let Err(e) = evm.stack.swap(2) {
            return ExecutionResult::Revert(e);
        }
        ExecutionResult::Halt
    }
    */

    fn op_lt(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on LT"),
        };
        let res = if a < b { 1 } else { 0 };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Revert("Stack Overflow on LT");
        }
        ExecutionResult::Halt
    }

    fn op_gt(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on GT"),
        };
        let res = if a > b { 1 } else { 0 };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Revert("Stack Overflow on GT");
        }
        ExecutionResult::Halt
    }

    fn op_eq(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on EQ"),
        };
        let res = if a == b { 1 } else { 0 };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Revert("Stack Overflow on EQ");
        }
        ExecutionResult::Halt
    }

    fn op_iszero(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.pop() {
            Ok(v) => v,
            _ => return ExecutionResult::Revert("Stack Underflow on ISZERO"),
        };
        let res = if val == 0 { 1 } else { 0 };
        if evm.stack.push(res).is_err() {
            return ExecutionResult::Revert("Stack Overflow on ISZERO");
        }
        ExecutionResult::Halt
    }

    fn op_and(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on AND"),
        };
        if evm.stack.push(a & b).is_err() {
            return ExecutionResult::Revert("Stack Overflow on AND");
        }
        ExecutionResult::Halt
    }

    fn op_or(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on OR"),
        };
        if evm.stack.push(a | b).is_err() {
            return ExecutionResult::Revert("Stack Overflow on OR");
        }
        ExecutionResult::Halt
    }

    fn op_xor(evm: &mut Evm) -> ExecutionResult {
        let (a, b) = match (evm.stack.pop(), evm.stack.pop()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return ExecutionResult::Revert("Stack Underflow on XOR"),
        };
        if evm.stack.push(a ^ b).is_err() {
            return ExecutionResult::Revert("Stack Overflow on XOR");
        }
        ExecutionResult::Halt
    }

    fn op_not(evm: &mut Evm) -> ExecutionResult {
        let val = match evm.stack.pop() {
            Ok(v) => v,
            _ => return ExecutionResult::Revert("Stack Underflow on NOT"),
        };
        if evm.stack.push(!val).is_err() {
            return ExecutionResult::Revert("Stack Overflow on NOT");
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
