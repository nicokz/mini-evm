// src/opcodes.rs

pub const STOP: u8 = 0x00;
pub const ADD: u8 = 0x01;
pub const MUL: u8 = 0x02;
pub const SUB: u8 = 0x03;
pub const DIV: u8 = 0x04;
pub const SDIV: u8 = 0x05;
pub const MOD: u8 = 0x06;
pub const SMOD: u8 = 0x07;
pub const ADDMOD: u8 = 0x08;
pub const MULMOD: u8 = 0x09;
pub const EXP: u8 = 0x0A;
pub const SIGNEXTEND: u8 = 0x0B;
pub const KECCAK256: u8 = 0x20;
pub const MLOAD: u8 = 0x51;
pub const MSTORE: u8 = 0x52;
pub const MSTORE8: u8 = 0x53;
pub const JUMP: u8 = 0x56;
pub const JUMPI: u8 = 0x57;
pub const MSIZE: u8 = 0x59;
pub const JUMPDEST: u8 = 0x5b;
pub const PUSH1: u8 = 0x60;
pub const PUSH2: u8 = 0x61;
//pub const DUP1: u8  = 0x80;
//pub const DUP2: u8  = 0x81;
//pub const SWAP1: u8 = 0x90;
//pub const SWAP2: u8 = 0x91;
pub const LT: u8 = 0x10;
pub const GT: u8 = 0x11;
pub const EQ: u8 = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8 = 0x16;
pub const OR: u8 = 0x17;
pub const XOR: u8 = 0x18;
pub const NOT: u8 = 0x19;
pub const BYTE: u8 = 0x1A;
pub const SHL: u8 = 0x1B;
pub const SHR: u8 = 0x1C;
pub const SAR: u8 = 0x1D;
pub const SLOAD: u8 = 0x54;
pub const SSTORE: u8 = 0x55;
pub const TLOAD: u8 = 0x5c;
pub const TSTORE: u8 = 0x5d;
pub const RETURN: u8 = 0xf3;
pub const REVERT: u8 = 0xfd;
pub const CALLDATALOAD: u8 = 0x35;
pub const CALLDATASIZE: u8 = 0x36;
pub const CALLDATACOPY: u8 = 0x37;
pub const CODECOPY: u8 = 0x39;
pub const ADDRESS: u8 = 0x30;
pub const BALANCE: u8 = 0x31;
pub const BLOCKHASH: u8 = 0x40;
pub const CALLER: u8 = 0x33;
pub const CALLVALUE: u8 = 0x34;
pub const CODESIZE: u8 = 0x38;
pub const EXTCODESIZE: u8 = 0x3B;
pub const EXTCODECOPY: u8 = 0x3C;
pub const GASPRICE: u8 = 0x3A;
pub const EXTCODEHASH: u8 = 0x3F;
pub const RETURNDATASIZE: u8 = 0x3D;
pub const RETURNDATACOPY: u8 = 0x3E;
pub const ORIGIN: u8 = 0x32;
pub const COINBASE: u8 = 0x41;
pub const TIMESTAMP: u8 = 0x42;
pub const NUMBER: u8 = 0x43;
pub const PREVRANDAO: u8 = 0x44;
pub const GASLIMIT: u8 = 0x45;
pub const CHAINID: u8 = 0x46;
pub const BASEFEE: u8 = 0x48;
pub const SELFBALANCE: u8 = 0x47;
pub const BLOBHASH: u8 = 0x49;
pub const BLOBBASEFEE: u8 = 0x4a;
pub const GAS: u8 = 0x5A;
pub const LOG0: u8 = 0xA0;
pub const LOG1: u8 = 0xA1;
pub const LOG2: u8 = 0xA2;
pub const LOG3: u8 = 0xA3;
pub const LOG4: u8 = 0xA4;
pub const CALL: u8 = 0xF1;
pub const CREATE: u8 = 0xF0;
pub const DELEGATECALL: u8 = 0xF4;
pub const CREATE2: u8 = 0xF5;
pub const STATICCALL: u8 = 0xFA;
pub const INVALID: u8 = 0xFE;
pub const SELFDESTRUCT: u8 = 0xFF;

macro_rules! define_opcodes {
    ($($name:ident = $val:expr);* $(;)?) => {
        $( pub const $name: u8 = $val; )*
    };
}

define_opcodes! {
    PUSH3 = 0x62; PUSH4 = 0x63; PUSH5 = 0x64; PUSH6 = 0x65;
    PUSH7 = 0x66; PUSH8 = 0x67; PUSH9 = 0x68; PUSH10 = 0x69;
    PUSH11 = 0x6A; PUSH12 = 0x6B; PUSH13 = 0x6C; PUSH14 = 0x6D;
    PUSH15 = 0x6E; PUSH16 = 0x6F; PUSH17 = 0x70; PUSH18 = 0x71;
    PUSH19 = 0x72; PUSH20 = 0x73; PUSH21 = 0x74; PUSH22 = 0x75;
    PUSH23 = 0x76; PUSH24 = 0x77; PUSH25 = 0x78; PUSH26 = 0x79;
    PUSH27 = 0x7A; PUSH28 = 0x7B; PUSH29 = 0x7C; PUSH30 = 0x7D;
    PUSH31 = 0x7E; PUSH32 = 0x7F;

    // DUP1..DUP16 (0x80..0x8F)
    DUP1 = 0x80;  DUP2 = 0x81;  DUP3 = 0x82;  DUP4 = 0x83;
    DUP5 = 0x84;  DUP6 = 0x85;  DUP7 = 0x86;  DUP8 = 0x87;
    DUP9 = 0x88;  DUP10 = 0x89; DUP11 = 0x8A; DUP12 = 0x8B;
    DUP13 = 0x8C; DUP14 = 0x8D; DUP15 = 0x8E; DUP16 = 0x8F;

    // SWAP1..SWAP16 (0x90..0x9F)
    SWAP1 = 0x90;  SWAP2 = 0x91;  SWAP3 = 0x92;  SWAP4 = 0x93;
    SWAP5 = 0x94;  SWAP6 = 0x95;  SWAP7 = 0x96;  SWAP8 = 0x97;
    SWAP9 = 0x98;  SWAP10 = 0x99; SWAP11 = 0x9A; SWAP12 = 0x9B;
    SWAP13 = 0x9C; SWAP14 = 0x9D; SWAP15 = 0x9E; SWAP16 = 0x9F;
}
