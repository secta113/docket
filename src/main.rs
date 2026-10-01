//! Checks the records an agent works from (backlog, specs and log in `docs/`, written in OKF 0.2) and writes their
//! index files.
//!
//! A port of the record checks in project-template (`tests/backlog_bundle.py` and its siblings), so that a project
//! that is not written in Python does not need Python only for its records.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Local;
use clap::{Parser, Subcommand};
use docket::bundle::{Bundle, backlog, stale};

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
    let result = match cli.command {
        Command::Check => check(&cli.root),
        Command::Index => index(&cli.root).map(|()| true),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        // A broken rule: 1. A file that could not be read or written: 2
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::from(2)
        }
    }
}

/// Print every broken rule under the check that found it. `true` when there are none.
fn check(root: &Path) -> Result<bool, String> {
    let found = docket::check::check(root).map_err(|e| e.to_string())?;
    let mut last = "";
    for finding in &found {
        if finding.check != last {
            println!("{}:", finding.check);
            last = finding.check;
        }
        println!("  {}", finding.detail);
    }
    if found.is_empty() {
        println!("every record keeps the rules");
    }
    Ok(found.is_empty())
}

/// Write every index file, then list what was left out of them and the items to measure again.
fn index(root: &Path) -> Result<(), String> {
    let bundle = Bundle::new(root);
    let (files, problems) = bundle.expected().map_err(|e| e.to_string())?;
    for (path, text) in files {
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", relative(&path, root));
    }
    for (name, why) in problems {
        println!("left out of the index, fix it: {name}: {why}");
    }
    let parsed = backlog(&bundle.read_folder("backlog").map_err(|e| e.to_string())?);
    for name in stale(&parsed.items, Local::now().fixed_offset()) {
        let at = parsed.items[&name].0.stale_after.unwrap();
        println!("past stale_after, measure the state again: {name} ({at})");
    }
    Ok(())
}

/// The path from the root, with `/` on every platform.
fn relative(path: &Path, root: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
