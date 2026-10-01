//! The CI entry point. Run `cargo xtask ci` both on a developer machine and in GitHub Actions, so that the two cannot
//! disagree about which checks ran.
//!
//! Cargo is the one that runs this (`$CARGO`), never another version on `PATH`. **A missing tool is a failure.**
//! Skipping it would let CI pass with a check silently gone.

use std::env;
use std::process::{Command, ExitCode};

const CHECKS: &[(&str, &[&str])] = &[
    ("Format (rustfmt)", &["fmt", "--all", "--check"]),
    // Warnings fail too, so they cannot pile up behind a green CI
    (
        "Lint (clippy)",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
    ("Tests (cargo test)", &["test", "--workspace"]),
];

fn run(cargo: &str, name: &str, args: &[&str]) -> bool {
    println!("\n--- {name} ---\n$ cargo {}", args.join(" "));
    match Command::new(cargo).args(args).status() {
        Ok(status) => status.success(),
        Err(e) => {
            println!("cargo could not run: {e}");
            false
        }
    }
}

fn main() -> ExitCode {
    let task = env::args().nth(1);
    if task.as_deref() != Some("ci") {
        eprintln!("usage: cargo xtask ci");
        return ExitCode::from(2);
    }
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let results: Vec<(&str, bool)> = CHECKS
        .iter()
        .map(|(name, args)| (*name, run(&cargo, name, args)))
        .collect();
    println!("\n{}", "=".repeat(40));
    for (name, ok) in &results {
        println!(" {name:<24}: {}", if *ok { "PASSED" } else { "FAILED" });
    }
    if results.iter().all(|(_, ok)| *ok) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
