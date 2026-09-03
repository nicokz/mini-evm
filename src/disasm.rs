use crate::opcodes::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub pc: usize,
    pub opcode: u8,
    pub mnemonic: &'static str,
    pub push_bytes: Option<Vec<u8>>,
}

pub fn opcode_mnemonic(opcode: u8) -> &'static str {
    match opcode {
        STOP => "STOP",
        ADD => "ADD",
        MUL => "MUL",
        SUB => "SUB",
        DIV => "DIV",
        SDIV => "SDIV",
        MOD => "MOD",
        SMOD => "SMOD",
        ADDMOD => "ADDMOD",
        MULMOD => "MULMOD",
        EXP => "EXP",
        SIGNEXTEND => "SIGNEXTEND",
        LT => "LT",
        GT => "GT",
        EQ => "EQ",
        ISZERO => "ISZERO",
        AND => "AND",
        OR => "OR",
        XOR => "XOR",
        NOT => "NOT",
        BYTE => "BYTE",
        SHL => "SHL",
        SHR => "SHR",
        SAR => "SAR",
        KECCAK256 => "KECCAK256",
        ADDRESS => "ADDRESS",
        ORIGIN => "ORIGIN",
        CALLER => "CALLER",
        CALLVALUE => "CALLVALUE",
        CALLDATALOAD => "CALLDATALOAD",
        CALLDATASIZE => "CALLDATASIZE",
        CALLDATACOPY => "CALLDATACOPY",
        CODESIZE => "CODESIZE",
        CODECOPY => "CODECOPY",
        GASPRICE => "GASPRICE",
        RETURNDATASIZE => "RETURNDATASIZE",
        RETURNDATACOPY => "RETURNDATACOPY",
        COINBASE => "COINBASE",
        TIMESTAMP => "TIMESTAMP",
        NUMBER => "NUMBER",
        PREVRANDAO => "PREVRANDAO",
        GASLIMIT => "GASLIMIT",
        CHAINID => "CHAINID",
        BASEFEE => "BASEFEE",
        BLOBHASH => "BLOBHASH",
        BLOBBASEFEE => "BLOBBASEFEE",
        MLOAD => "MLOAD",
        MSTORE => "MSTORE",
        MSTORE8 => "MSTORE8",
        SLOAD => "SLOAD",
        SSTORE => "SSTORE",
        JUMP => "JUMP",
        JUMPI => "JUMPI",
        MSIZE => "MSIZE",
        GAS => "GAS",
        JUMPDEST => "JUMPDEST",
        CREATE => "CREATE",
        CALL => "CALL",
        DELEGATECALL => "DELEGATECALL",
        CREATE2 => "CREATE2",
        STATICCALL => "STATICCALL",
        RETURN => "RETURN",
        REVERT => "REVERT",
        0x80..=0x8f => dup_mnemonic(opcode - 0x7f),
        0x90..=0x9f => swap_mnemonic(opcode - 0x8f),
        0xa0..=0xa4 => log_mnemonic(opcode - 0xa0),
        0x60..=0x7f => push_mnemonic(opcode - 0x5f),
        _ => "INVALID",
    }
}

fn push_mnemonic(size: u8) -> &'static str {
    const NAMES: [&str; 32] = [
        "PUSH1", "PUSH2", "PUSH3", "PUSH4", "PUSH5", "PUSH6", "PUSH7", "PUSH8", "PUSH9", "PUSH10",
        "PUSH11", "PUSH12", "PUSH13", "PUSH14", "PUSH15", "PUSH16", "PUSH17", "PUSH18", "PUSH19",
        "PUSH20", "PUSH21", "PUSH22", "PUSH23", "PUSH24", "PUSH25", "PUSH26", "PUSH27", "PUSH28",
        "PUSH29", "PUSH30", "PUSH31", "PUSH32",
    ];
    NAMES[(size - 1) as usize]
}

fn dup_mnemonic(index: u8) -> &'static str {
    const NAMES: [&str; 16] = [
        "DUP1", "DUP2", "DUP3", "DUP4", "DUP5", "DUP6", "DUP7", "DUP8", "DUP9", "DUP10", "DUP11",
        "DUP12", "DUP13", "DUP14", "DUP15", "DUP16",
    ];
    NAMES[(index - 1) as usize]
}

fn swap_mnemonic(index: u8) -> &'static str {
    const NAMES: [&str; 16] = [
        "SWAP1", "SWAP2", "SWAP3", "SWAP4", "SWAP5", "SWAP6", "SWAP7", "SWAP8", "SWAP9", "SWAP10",
        "SWAP11", "SWAP12", "SWAP13", "SWAP14", "SWAP15", "SWAP16",
    ];
    NAMES[(index - 1) as usize]
}

fn log_mnemonic(index: u8) -> &'static str {
    ["LOG0", "LOG1", "LOG2", "LOG3", "LOG4"][index as usize]
}

pub fn disassemble(code: &[u8]) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    let mut pc = 0;
    while pc < code.len() {
        let opcode = code[pc];
        let mnemonic = opcode_mnemonic(opcode);
        let push_bytes = if (PUSH1..=PUSH32).contains(&opcode) {
            let size = (opcode - PUSH1 + 1) as usize;
            let available = code.len().saturating_sub(pc + 1).min(size);
            Some(code[pc + 1..pc + 1 + available].to_vec())
        } else {
            None
        };
        let immediate_len = push_bytes.as_ref().map_or(0, Vec::len);
        instructions.push(Instruction {
            pc,
            opcode,
            mnemonic,
            push_bytes,
        });
        pc += 1 + immediate_len;
        if (PUSH1..=PUSH32).contains(&opcode) && immediate_len < (opcode - PUSH1 + 1) as usize {
            break;
        }
    }
    instructions
}
