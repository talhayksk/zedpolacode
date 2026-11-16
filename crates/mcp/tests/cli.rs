//! Integration tests for the `card` command line.
//!
//! The parser is unit-tested in `src/cli.rs`; these two spawn the real binary
//! because the contract a keybinding depends on is the process exit code and
//! what it prints. Nothing here needs a browser: help short-circuits before
//! discovery, and a bad flag stops at the parser.

use std::process::{Command, Output, Stdio};

/// Cargo builds the binary before it runs integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_polacode-mcp");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("the binary starts")
}

#[test]
fn card_help_prints_the_usage_and_exits_zero() {
    let output = run(&["card", "--help"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("Usage:"), "{stdout}");
    assert!(stdout.contains("polacode-mcp card --clipboard"), "{stdout}");
    assert!(stdout.contains("polacode-mcp card --file"), "{stdout}");
}

#[test]
fn an_unknown_card_option_still_exits_two() {
    let output = run(&["card", "--badflag"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("--badflag"), "{stderr}");
    assert!(stderr.contains("polacode-mcp --help"), "{stderr}");
    assert!(output.stdout.is_empty(), "nothing on stdout");
}
