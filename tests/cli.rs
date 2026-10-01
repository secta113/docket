//! The command line, run as a user runs it: the built binary in a separate process.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docket"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repository with an empty bundle: the backlog rules, and the spec directories.
fn repo() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let docs = root.path().join("docs");
    for folder in ["backlog", "specs", "done"] {
        fs::create_dir_all(docs.join(folder)).unwrap();
    }
    fs::write(
        docs.join("backlog/rules.md"),
        "---\ntype: Guide\ntitle: Backlog rules\ndescription: What goes here.\n---\n\n# What goes here\n",
    )
    .unwrap();
    root
}

fn root_arg(root: &Path) -> String {
    root.to_string_lossy().into_owned()
}

#[test]
fn a_missing_root_fails() {
    let out = run(&["--root", "no/such/directory", "check"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a directory"));
}

#[test]
fn an_unimplemented_command_fails() {
    let root = repo();
    let out = run(&["--root", &root_arg(root.path()), "check"]);
    assert!(!out.status.success(), "check passed with nothing checked");
}

#[test]
fn index_writes_every_index_file() {
    let root = repo();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for path in [
        "docs/index.md",
        "docs/backlog/index.md",
        "docs/specs/index.md",
        "docs/done/index.md",
    ] {
        assert!(root.path().join(path).is_file(), "{path} was not written");
        assert!(
            stdout(&out).contains(&format!("wrote {path}")),
            "{}",
            stdout(&out)
        );
    }
    let backlog = fs::read_to_string(root.path().join("docs/backlog/index.md")).unwrap();
    assert!(
        backlog.contains("* [Backlog rules](rules.md) - What goes here."),
        "{backlog}"
    );
}

#[test]
fn index_names_what_it_left_out() {
    let root = repo();
    fs::write(
        root.path().join("docs/backlog/broken.md"),
        "# no frontmatter\n",
    )
    .unwrap();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("left out of the index, fix it: broken.md: no frontmatter"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn index_names_the_items_to_measure_again() {
    let root = repo();
    let item = "---
type: Backlog Item
title: Old
description: Measured long ago.
tags: [operations]
status: stable
filed: 2020-01-01
verified: {by: human:someone, at: 2020-01-02T10:00:00+09:00}
stale_after: 2021-01-01T00:00:00+09:00
deadline_kind: none
deadline: an alarm
---

# Trigger

X.

# State

Y.

# Details

Z.
";
    fs::write(root.path().join("docs/backlog/old.md"), item).unwrap();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("past stale_after, measure the state again: old.md"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn index_fails_without_a_spec_directory() {
    let root = repo();
    fs::remove_dir(root.path().join("docs/done")).unwrap();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("done"));
}
