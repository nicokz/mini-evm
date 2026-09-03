# mini-evm

`mini-evm` is a small, educational Ethereum Virtual Machine implementation written in Rust. It can execute EVM bytecode as a library, inspect bytecode from a command line, and expose a lightweight JSON-RPC server for local development and testing.

The project is intentionally compact and readable. It is useful for learning EVM execution, experimenting with opcode and gas behavior, building deterministic fixtures, and testing clients against a local in-memory node. It is not intended to be a production Ethereum client or a consensus implementation.

## Features

- EVM bytecode execution with a 256-bit stack and bounded memory growth.
- Arithmetic, comparison, bitwise, environmental, calldata, memory, storage, control-flow, logging, and return/revert operations.
- Contract calls and creation operations, including `CALL`, `DELEGATECALL`, `STATICCALL`, `CREATE`, and `CREATE2`.
- Nested call frames with call context, return data, static-call checks, and revert handling.
- Transaction and account state with balances, nonces, code, storage, snapshots, and state diffs.
- Gas accounting including memory expansion and dynamic storage access pricing.
- Cancun-era behavior covered by the test suite, including transient storage (`TLOAD`/`TSTORE`), `MCOPY`, blob hashes, blob base fee, and `SELFDESTRUCT` lifecycle behavior.
- Keccak-256, SHA-256, RIPEMD-160, and secp256k1-related cryptographic utilities and precompiles.
- Merkle Patricia Trie encoding and state-root calculation.
- Bytecode disassembly and step-by-step execution tracing.
- An in-memory JSON-RPC server with transaction pool, block mining, receipts, gas estimation, and WebSocket subscriptions.
- Parallel fuzzing of calldata against a bytecode program through the `/v1/fuzz` endpoint.

## Project Status

This is an experimental implementation for education, testing, and local simulation. EVM and JSON-RPC compatibility is intentionally incomplete, and behavior may differ from a full Ethereum client. Do not use it to secure funds, validate mainnet consensus, or expose an untrusted network service.

## Requirements

- Rust stable with Cargo

The crate uses Rust edition 2024. Dependencies are declared in [`Cargo.toml`](Cargo.toml), and the repository includes vendored dependencies for reproducible local builds in environments configured to use them.

## Quick Start

Clone and build the project:

```bash
git clone https://github.com/<your-account>/mini-evm.git
cd mini-evm
cargo build
```

Run the test suite:

```bash
cargo test
```

Execute a small bytecode program. This program pushes `5` and `7`, adds them, and stops:

```bash
cargo run --bin mini-evm-cli -- 0x600560070100
```

The following is the complete valid example for `PUSH1 5 PUSH1 7 ADD STOP`:

```bash
cargo run --bin mini-evm-cli -- 0x600560070100
```

The command prints the execution result and final stack. The CLI accepts bytecode with or without the `0x` prefix.

## Command-Line Tool

`mini-evm-cli` is a small bytecode inspection and execution tool:

```text
Usage: mini-evm-cli [OPTIONS] <HEX_BYTECODE>

Options:
      --disasm
      --trace
      --gas <GAS>  [default: 30000000]
```

Disassemble bytecode without executing it:

```bash
cargo run --bin mini-evm-cli -- --disasm 0x6005600701600055
```

Execute with opcode-level tracing:

```bash
cargo run --bin mini-evm-cli -- --trace --gas 100000 0x600560070100
```

The trace includes the program counter, opcode, remaining gas, opcode gas cost, and stack contents after each step.

## Rust Library

The main public modules are exported from [`src/lib.rs`](src/lib.rs). The simplest way to execute bytecode is to construct an `Evm` directly:

```rust
use mini_evm::opcodes::{ADD, PUSH1, STOP};
use mini_evm::vm::{Evm, ExecutionResult};

let code = [PUSH1, 5, PUSH1, 7, ADD, STOP];
let mut evm = Evm::new(&code);

assert_eq!(evm.run(), ExecutionResult::Halt);
assert_eq!(evm.stack.pop().unwrap(), 12);
```

For transaction context, gas, and block environment configuration, use `EvmBuilder`:

```rust
use mini_evm::vm::{EvmBuilder, ExecutionResult};

let mut evm = EvmBuilder::new()
    .with_code(vec![0x60, 0x2a, 0x00]) // PUSH1 42, STOP
    .with_gas(100_000)
    .build();

assert_eq!(evm.run(), ExecutionResult::Halt);
```

The [`simulate_tx`](src/simulation.rs) helper executes against a cloned [`StateFork`](src/state.rs), reports gas use and logs, and returns dirty account state as a state diff. Failed executions revert the simulated state changes.

## Local JSON-RPC Server

Start the in-memory RPC server on the default port:

```bash
cargo run --bin mini-evm-rpc
```

The server listens on `127.0.0.1:8545`. Select another port with:

```bash
cargo run --bin mini-evm-rpc -- --port 18545
```

HTTP JSON-RPC requests are sent to `/`:

```bash
curl -s http://127.0.0.1:8545/ \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
```

The default server starts with an empty in-memory state. It does not persist accounts, blocks, transactions, or receipts between runs.

### Supported HTTP Methods

The current HTTP endpoint handles:

- `eth_call`
- `eth_estimateGas`
- `eth_getStorageAt`
- `eth_getCode`
- `eth_sendRawTransaction`
- `eth_blockNumber`
- `eth_getBlockByNumber`
- `eth_getTransactionReceipt`
- `eth_pendingTransactions`
- `evm_mine`
- `miner_mine` (alias for `evm_mine`)
- `txpool_status`
- `net_version`
- `eth_chainId`

Example read-only call against the empty state:

```bash
curl -s http://127.0.0.1:8545/ \
  -H 'content-type: application/json' \
  --data '{
    "jsonrpc":"2.0",
    "id":1,
    "method":"eth_call",
    "params":[
      {"to":"0x0000000000000000000000000000000000000000","data":"0x"},
      "latest"
    ]
  }'
```

Mine an empty block:

```bash
curl -s http://127.0.0.1:8545/ \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":2,"method":"evm_mine","params":[]}'
```

### WebSocket Subscriptions

Connect to `ws://127.0.0.1:8545/ws` and use JSON-RPC subscription messages. Supported subscription kinds are:

- `newHeads`
- `pendingTransactions`
- `logs`, optionally with an address and topic filter

For example, the subscription request for new block headers is:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_subscribe",
  "params": ["newHeads"]
}
```

Unsubscribe with `eth_unsubscribe` and the returned subscription id.

### Calldata Fuzzing

The `POST /v1/fuzz` endpoint runs deterministic, parallel calldata mutations against supplied bytecode. It reports either `CLEAN` or `INVARIANT_BREACH` when execution returns an EVM error.

```bash
curl -s http://127.0.0.1:8545/v1/fuzz \
  -H 'content-type: application/json' \
  --data '{
    "bytecode_hex":"0x600560070100",
    "iterations":1000,
    "gas_limit":3000000
  }'
```

The endpoint accepts up to 10,000,000 iterations, a non-zero gas limit, and bytecode up to 1 MiB. The `gas_limit` field defaults to `3,000,000` when omitted.

## Repository Layout

| Path | Purpose |
| --- | --- |
| [`src/vm.rs`](src/vm.rs) | EVM state, execution dispatch, gas, and opcode behavior |
| [`src/vm/`](src/vm) | Execution helpers, call frames, and transaction application |
| [`src/opcodes.rs`](src/opcodes.rs) | Opcode constants |
| [`src/stack.rs`](src/stack.rs) | 256-bit stack implementation |
| [`src/state.rs`](src/state.rs) | Account state, snapshots, transient storage, and state roots |
| [`src/mpt/`](src/mpt) | Merkle Patricia Trie implementation |
| [`src/tx/`](src/tx) | Transaction encoding and handling |
| [`src/rpc/`](src/rpc) | JSON-RPC handlers, types, estimator, and server |
| [`src/sim/`](src/sim) | Simulation helpers and scenario engines |
| [`src/disasm.rs`](src/disasm.rs) | Bytecode disassembler |
| [`src/tracer.rs`](src/tracer.rs) | Per-step execution tracing |
| [`src/fuzzer.rs`](src/fuzzer.rs) | Deterministic parallel calldata fuzzing |
| [`tests/`](tests) | Integration tests for EVM, RPC, transactions, and protocol behavior |

## Testing

Run all unit and integration tests:

```bash
cargo test
```

Run tests with output from individual test cases:

```bash
cargo test -- --nocapture
```

The integration suite includes opcode behavior, gas rules, calls and contract lifecycle, transient storage, blob operations, state roots, transaction handling, JSON-RPC, WebSockets, and simulation workflows.

## Design Notes and Limitations

- State is in memory and is initialized empty by the RPC binary.
- The RPC server is bound to loopback by default and has no authentication or persistence layer.
- The supported opcode and JSON-RPC surfaces are intentionally smaller than those of a full Ethereum client.
- Chain metadata exposed by the current RPC server is fixed to chain id `1` and network version `1`.
- Block production is a local simulation: block headers, timestamps, mining, and transaction pool behavior are simplified.
- The implementation should be treated as a learning and testing aid, not as a consensus-critical execution engine.

## Contributing

Small, focused changes are welcome. Before opening a pull request:

```bash
cargo fmt -- --check
cargo test
```

When adding an opcode or protocol behavior, include a focused unit or integration test and document any intentionally simplified semantics.

## License

No license file is currently included.