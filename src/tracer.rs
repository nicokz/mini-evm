use ruint::aliases::U256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFrame {
    pub pc: usize,
    pub op: u8,
    pub mnemonic: &'static str,
    pub gas_left: u64,
    pub gas_cost: u64,
    pub stack: Vec<U256>,
    pub memory: Vec<u8>,
}

pub type StepTracer = Box<dyn FnMut(&StepFrame)>;
