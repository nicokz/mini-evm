use clap::Parser;
use colored::Colorize;
use mini_evm::disasm::{Instruction, disassemble};
use mini_evm::tracer::StepFrame;
use mini_evm::vm::Evm;
use ruint::aliases::U256;

#[derive(Debug, Parser)]
#[command(name = "mini-evm-cli", about = "Inspect and execute EVM bytecode")]
struct Args {
    #[arg(value_name = "HEX_BYTECODE")]
    bytecode: String,

    #[arg(long)]
    disasm: bool,

    #[arg(long)]
    trace: bool,

    #[arg(long, default_value_t = 30_000_000)]
    gas: u64,
}

fn format_u256(value: &U256) -> String {
    let bytes = value.to_be_bytes::<32>();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(31);
    format!("0x{}", hex::encode(&bytes[first..]))
}

fn format_instruction(instruction: &Instruction) -> String {
    let immediate = instruction
        .push_bytes
        .as_ref()
        .map(|bytes| format!(" 0x{}", hex::encode(bytes)))
        .unwrap_or_default();
    format!(
        "{:04x} | {}{}",
        instruction.pc, instruction.mnemonic, immediate
    )
}

fn print_trace(frame: &StepFrame, code: &[u8]) {
    let stack = frame
        .stack
        .iter()
        .map(format_u256)
        .collect::<Vec<_>>()
        .join(", ");
    let mnemonic = if (0x60..=0x7f).contains(&frame.op) {
        let size = (frame.op - 0x5f) as usize;
        let available = code.len().saturating_sub(frame.pc + 1).min(size);
        format!(
            "{} 0x{}",
            frame.mnemonic,
            hex::encode(&code[frame.pc + 1..frame.pc + 1 + available])
        )
    } else {
        frame.mnemonic.to_string()
    };
    println!(
        "{} | {} {:<14} | {} {} (-{}) | {} [{}]",
        format!("[PC: {:04x}]", frame.pc).cyan(),
        "OP:".dimmed(),
        mnemonic.yellow(),
        "GAS:".dimmed(),
        frame.gas_left,
        frame.gas_cost,
        "STACK:".dimmed(),
        stack
    );
}

fn main() {
    let args = Args::parse();
    let encoded = args.bytecode.strip_prefix("0x").unwrap_or(&args.bytecode);
    let code = match hex::decode(encoded) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("invalid bytecode: {error}");
            std::process::exit(2);
        }
    };

    if args.disasm {
        for instruction in disassemble(&code) {
            println!("{}", format_instruction(&instruction));
        }
        return;
    }

    let mut vm = Evm::new_with_gas(&code, args.gas);
    if args.trace {
        let trace_code = code.clone();
        vm.tracer = Some(Box::new(move |frame| print_trace(frame, &trace_code)));
    }
    let result = vm.run();
    println!("RESULT: {:?}", result);
    let stack = vm
        .stack
        .values()
        .iter()
        .map(format_u256)
        .collect::<Vec<_>>()
        .join(", ");
    println!("FINAL STACK: [{}]", stack);
}
