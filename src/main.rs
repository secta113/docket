//! Checks the records an agent works from (backlog, specs and log in `docs/`, written in OKF 0.2) and writes their
//! index files.
//!
//! A port of the record checks in project-template (`tests/backlog_bundle.py` and its siblings), so that a project
//! that is not written in Python does not need Python only for its records.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The repository root. The bundle is `<root>/docs`
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check the records, and exit non-zero when one breaks the rules
    Check,
    /// Write every index.md in the bundle from the frontmatter
    Index,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // A root that does not exist fails here. Read as an empty tree, it would pass every check with nothing checked
    if !cli.root.is_dir() {
        eprintln!("the root is not a directory: {}", cli.root.display());
        return ExitCode::from(2);
    }
    match cli.command {
        // Not implemented yet, so it fails: a stub that exits 0 would look like a passing check
        Command::Check | Command::Index => {
            eprintln!("not implemented yet");
            ExitCode::from(2)
        }
    }
}
