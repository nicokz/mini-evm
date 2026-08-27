// src/opcodes.rs

pub const STOP: u8     = 0x00;
pub const ADD: u8      = 0x01;
pub const SUB: u8      = 0x03;
pub const MLOAD: u8    = 0x51;
pub const MSTORE: u8   = 0x52;
pub const JUMP: u8     = 0x56;
pub const JUMPI: u8    = 0x57;
pub const MSIZE: u8    = 0x59;
pub const JUMPDEST: u8 = 0x5b;
pub const PUSH1: u8    = 0x60;
pub const DUP1: u8  = 0x80;
pub const DUP2: u8  = 0x81;
pub const SWAP1: u8 = 0x90;
pub const SWAP2: u8 = 0x91;
pub const LT: u8     = 0x10;
pub const GT: u8     = 0x11;
pub const EQ: u8     = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8    = 0x16;
pub const OR: u8     = 0x17;
pub const XOR: u8    = 0x18;
pub const NOT: u8    = 0x19;