use crate::crypto::keccak256;
use crate::state::StateFork;
use crate::vm::{Evm, ExecutionResult};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const MAX_ITERATIONS: u64 = 10_000_000;
const MAX_BYTECODE: usize = 1 << 20;

#[derive(Debug, Deserialize)]
pub struct FuzzRequest {
    pub bytecode_hex: String,
    pub iterations: u64,
    #[serde(default = "default_gas_limit")]
    pub gas_limit: u64,
}

fn default_gas_limit() -> u64 {
    3_000_000
}

#[derive(Debug, Serialize)]
pub struct FuzzReport {
    pub status: String,
    pub iterations_executed: u64,
    pub violation_found: bool,
    pub payload_hex: Option<String>,
    pub error_log: Option<String>,
}

impl FuzzReport {
    pub fn error(error: String) -> Self {
        Self {
            status: "ERROR".into(),
            iterations_executed: 0,
            violation_found: false,
            payload_hex: None,
            error_log: Some(error),
        }
    }
}

impl FuzzRequest {
    pub fn run(&self, base_state: &StateFork) -> Result<FuzzReport, String> {
        if self.iterations > MAX_ITERATIONS || self.gas_limit == 0 {
            return Err("invalid fuzz limits".into());
        }
        let bytecode = hex::decode(
            self.bytecode_hex
                .strip_prefix("0x")
                .unwrap_or(&self.bytecode_hex),
        )
        .map_err(|e| e.to_string())?;
        if bytecode.len() > MAX_BYTECODE {
            return Err("bytecode exceeds 1 MiB".into());
        }
        let jobs: Vec<u64> = (0..self.iterations).collect();
        let first = jobs.par_iter().find_map_any(|iteration| {
            let payload = mutate(*iteration);
            let target = [0x42; 20];
            let mut vm = Evm::new_with_gas(&bytecode, self.gas_limit);
            vm.state = base_state.clone();
            vm.context.address = target;
            vm.context.calldata = payload.clone();
            let result = vm.run();
            if matches!(
                result,
                ExecutionResult::Error(_) | ExecutionResult::VmError(_)
            ) {
                Some((
                    *iteration,
                    payload,
                    format!("execution failure: {result:?}"),
                ))
            } else {
                None
            }
        });
        Ok(match first {
            Some((iteration, payload, error)) => FuzzReport {
                status: "INVARIANT_BREACH".into(),
                iterations_executed: iteration + 1,
                violation_found: true,
                payload_hex: Some(hex::encode(payload)),
                error_log: Some(error),
            },
            None => FuzzReport {
                status: "CLEAN".into(),
                iterations_executed: self.iterations,
                violation_found: false,
                payload_hex: None,
                error_log: None,
            },
        })
    }
}

fn mutate(iteration: u64) -> Vec<u8> {
    let hash = keccak256(&iteration.to_le_bytes());
    let mut payload = Vec::with_capacity(68);
    payload.extend_from_slice(&hash[..4]);
    payload.extend_from_slice(&hash[4..32]);
    payload.extend_from_slice(&hash[..28]);
    payload
}
