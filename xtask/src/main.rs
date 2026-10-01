//! The CI entry point. Run `cargo xtask ci` both on a developer machine and in GitHub Actions, so that the two cannot
//! disagree about which checks ran.
//!
//! Cargo is the one that runs this (`$CARGO`), never another version on `PATH`. **A missing tool is a failure.**
//! Skipping it would let CI pass with a check silently gone.

mod drift;

use std::env;
use std::fs;
use std::path::Path;
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
    // Every test binary runs even after one fails: by default a failing unit test hides the command-line tests
    (
        "Tests (cargo test)",
        &["test", "--workspace", "--no-fail-fast"],
    ),
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

/// Print what an in-process check found. An error reading its input is a failure, never a pass
fn report(name: &str, problems: Result<Vec<String>, String>) -> bool {
    println!("\n--- {name} ---");
    let problems = problems.unwrap_or_else(|e| vec![e]);
    for p in &problems {
        println!("{p}");
    }
    problems.is_empty()
}

fn read(root: &Path, file: &str) -> Result<String, String> {
    fs::read_to_string(root.join(file)).map_err(|e| format!("cannot read {file}: {e}"))
}

/// The tracked files, `/`-separated. In the CI container the checkout belongs to another user, and git refuses to read
/// such a repository unless it is marked safe; this only reads, so it is marked safe for this one command
fn tracked(root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(["-c", "safe.directory=*", "ls-files"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

fn map(root: &Path) -> Result<Vec<String>, String> {
    let map = drift::read_map(&read(root, "AGENTS.md")?);
    Ok(drift::map_problems(&map, &tracked(root)?))
}

fn toolchain(root: &Path) -> Result<Vec<String>, String> {
    Ok(drift::toolchain_problems(
        &read(root, "rust-toolchain.toml")?,
        &read(root, "Dockerfile")?,
        &read(root, ".github/workflows/ci.yml")?,
    ))
}

fn main() -> ExitCode {
    let task = env::args().nth(1);
    if task.as_deref() != Some("ci") {
        eprintln!("usage: cargo xtask ci");
        return ExitCode::from(2);
    }
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits inside the repository");
    let mut results: Vec<(&str, bool)> = CHECKS
        .iter()
        .map(|(name, args)| (*name, run(&cargo, name, args)))
        .collect();
    results.push(("Map (AGENTS.md)", report("Map (AGENTS.md)", map(root))));
    results.push((
        "Toolchain version",
        report("Toolchain version", toolchain(root)),
    ));
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
