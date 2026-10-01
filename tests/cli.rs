//! The command line, run as a user runs it: the built binary in a separate process.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docket"))
        .args(args)
        .output()
        .expect("the binary runs")
}

#[test]
fn a_missing_root_fails() {
    let out = run(&["--root", "no/such/directory", "check"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a directory"));
}

#[test]
fn an_unimplemented_command_fails() {
    for command in ["check", "index"] {
        assert!(
            !run(&[command]).status.success(),
            "{command} passed with nothing checked"
        );
    }
}
