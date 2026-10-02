//! Keeps a project's structure: makes and checks its layers (as `.config/docket.toml` declares them) and the records an
//! agent works from (backlog, specs and log in `docs/`, written in OKF 0.2), and writes their index files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Local;
use clap::{Parser, Subcommand};
use docket::bundle::{Bundle, backlog, stale};
use docket::source::relative_path;

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
    /// Write .config/docket.toml for a stack, once. Edit it, then run `docket create`
    Init {
        /// python, typescript, rust, or none (records only)
        #[arg(long)]
        stack: String,
    },
    /// Make the layers that .config/docket.toml declares and the tree lacks, and the records skeleton in docs/
    Create,
    /// Check the layers and the records, and exit non-zero when one breaks the rules
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
        Command::Init { stack } => init(&cli.root, &stack).map(|()| true),
        Command::Create => create(&cli.root).map(|()| true),
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

/// Write the declaration, and say what to do next.
fn init(root: &Path, stack: &str) -> Result<(), String> {
    let path = docket::init::init(root, stack)?;
    println!("wrote {path}");
    if stack == docket::layers::RECORDS_ONLY {
        println!("next: run `docket create` to make docs/");
    } else {
        println!(
            "next: declare in absent the layers this project does not have, then run `docket create`"
        );
    }
    Ok(())
}

/// Make what is missing, and say what was written. A second run with nothing changed writes nothing.
fn create(root: &Path) -> Result<(), String> {
    let made = docket::create::create(root)?;
    for path in &made.written {
        println!("wrote {path}");
    }
    if made.written.is_empty() {
        println!(
            "nothing to make: the tree has what {} declares",
            docket::layers::DECLARATION
        );
    }
    for (name, why) in made.left_out {
        println!("left out of the index, fix it: {name}: {why}");
    }
    Ok(())
}

/// Print every broken rule under the check that found it. `true` when there are none.
fn check(root: &Path) -> Result<bool, String> {
    let report = docket::check::check(root).map_err(|e| e.to_string())?;
    for why in &report.skipped {
        println!("{why}");
    }
    let found = report.findings;
    let mut last = "";
    for finding in &found {
        if finding.check != last {
            println!("{}:", finding.check);
            last = finding.check;
        }
        // A detail of several lines (an example to write) stays under its finding
        println!("  {}", finding.detail.replace('\n', "\n    "));
    }
    if found.is_empty() {
        match (report.layers_checked, report.skipped.is_empty()) {
            (true, true) => println!("the layers and every record keep the rules"),
            // What was not checked is printed above
            (true, false) => {
                println!("what was checked of the layers, and every record, keep the rules")
            }
            (false, _) => println!("every record keeps the rules"),
        }
    }
    Ok(found.is_empty())
}

/// Write every index file, then list what was left out of them and the items to measure again.
fn index(root: &Path) -> Result<(), String> {
    let areas = docket::layers::areas(root).map_err(|e| e.to_string())??;
    let bundle = Bundle::new(root, areas);
    let (files, problems) = bundle.expected().map_err(|e| e.to_string())?;
    for (path, text) in files {
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", relative_path(&path, root));
    }
    for (name, why) in problems {
        println!("left out of the index, fix it: {name}: {why}");
    }
    let docs = bundle.read_folder("backlog").map_err(|e| e.to_string())?;
    let parsed = backlog(&docs, &bundle.areas);
    for name in stale(&parsed.items, Local::now().fixed_offset()) {
        let at = parsed.items[&name].0.stale_after.unwrap();
        println!("past stale_after, measure the state again: {name} ({at})");
    }
    Ok(())
}
