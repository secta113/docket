//! The CI entry point. Run `cargo xtask ci` both on a developer machine and in GitHub Actions, so that the two cannot
//! disagree about which checks ran.
//!
//! Cargo is the one that runs this (`$CARGO`), never another version on `PATH`. **A missing tool is a failure.**
//! Skipping it would let CI pass with a check silently gone.

mod drift;

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Each check with how long it may run: about five times its slowest run in CI, and at least a minute. A check that
/// hangs fails with its name, instead of holding CI until the job's own limit
const CHECKS: &[(&str, Duration, &[&str])] = &[
    (
        "Format (rustfmt)",
        Duration::from_mins(1),
        &["fmt", "--all", "--check"],
    ),
    // Warnings fail too, so they cannot pile up behind a green CI
    (
        "Lint (clippy)",
        Duration::from_mins(2),
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
        Duration::from_mins(2),
        &["test", "--workspace", "--no-fail-fast"],
    ),
];

/// How long `git ls-files` may run. It takes milliseconds
const GIT_LIMIT: Duration = Duration::from_mins(1);

fn run(cargo: &str, name: &str, limit: Duration, args: &[&str]) -> bool {
    println!("\n--- {name} ---\n$ cargo {}", args.join(" "));
    let status = Command::new(cargo)
        .args(args)
        .spawn()
        .map_err(|e| format!("cargo could not run: {e}"))
        .and_then(|mut child| wait(&mut child, name, limit));
    match status {
        Ok(status) => status.success(),
        Err(e) => {
            println!("{e}");
            false
        }
    }
}

/// Wait for a child, but no longer than `limit`; then it is stopped and the step fails. Only the child is stopped, not
/// what it started: the job's own limit stops the rest
fn wait(child: &mut Child, name: &str, limit: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("{name} could not be waited for: {e}"))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let secs = limit.as_secs();
            return Err(match child.kill().and_then(|()| child.wait()) {
                Ok(_) => format!("{name} did not finish within {secs} seconds, and was stopped"),
                Err(e) => format!(
                    "{name} did not finish within {secs} seconds, and could not be stopped: {e}"
                ),
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Read a pipe to its end on another thread, so that a child never stops on a full pipe while it is waited for
fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Result<Vec<u8>, String>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| format!("cannot read the output of git: {e}"))
    })
}

fn drained(reader: thread::JoinHandle<Result<Vec<u8>, String>>) -> Result<Vec<u8>, String> {
    reader.join().expect("reading a pipe does not panic")
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
    let mut child = Command::new("git")
        .args(["-c", "safe.directory=*", "ls-files"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git could not run: {e}"))?;
    let stdout = drain(child.stdout.take().expect("stdout is piped"));
    let stderr = drain(child.stderr.take().expect("stderr is piped"));
    let status = wait(&mut child, "git ls-files", GIT_LIMIT)?;
    let (stdout, stderr) = (drained(stdout)?, drained(stderr)?);
    if !status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&stdout)
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
        .map(|(name, limit, args)| (*name, run(&cargo, name, *limit, args)))
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
