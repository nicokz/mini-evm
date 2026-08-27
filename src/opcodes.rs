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
//pub const DUP1: u8  = 0x80;
//pub const DUP2: u8  = 0x81;
//pub const SWAP1: u8 = 0x90;
//pub const SWAP2: u8 = 0x91;
pub const LT: u8     = 0x10;
pub const GT: u8     = 0x11;
pub const EQ: u8     = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8    = 0x16;
pub const OR: u8     = 0x17;
pub const XOR: u8    = 0x18;
pub const NOT: u8    = 0x19;

macro_rules! define_opcodes {
    ($($name:ident = $val:expr);* $(;)?) => {
        $( pub const $name: u8 = $val; )*
    };
}

define_opcodes! {
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