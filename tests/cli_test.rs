use std::process::Command;

#[test]
fn cli_trace_prints_execution_and_final_stack() {
    let output = Command::new(env!("CARGO_BIN_EXE_mini-evm-cli"))
        .args(["6005600a01", "--trace"])
        .output()
        .expect("failed to execute mini-evm-cli");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("CLI output was not UTF-8");
    assert!(stdout.contains("PUSH1 0x05"));
    assert!(stdout.contains("PUSH1 0x0a"));
    assert!(stdout.contains("ADD"));
    assert!(stdout.contains("0x0f"));
}
