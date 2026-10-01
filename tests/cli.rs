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

/// A repository that keeps every rule: the bundle, a log, and the index files written by `docket index`.
fn clean_repo() -> tempfile::TempDir {
    let root = repo();
    fs::write(
        root.path().join("docs/log.md"),
        "# Log\n\n## 2026-10-02\n\n### Something\n",
    )
    .unwrap();
    assert!(
        run(&["--root", &root_arg(root.path()), "index"])
            .status
            .success()
    );
    root
}

#[test]
fn a_clean_repository_passes() {
    let root = clean_repo();
    let out = run(&["--root", &root_arg(root.path()), "check"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("every record keeps the rules"));
}

#[test]
fn each_broken_rule_fails_under_its_check() {
    let item = |details: &str| {
        format!(
            "---\ntype: Backlog Item\ntitle: X\ndescription: Y.\ntags: [a]\nstatus: stable\nfiled: 2026-10-01\n\
             verified: {{by: human:a, at: 2026-10-01T10:00:00+09:00}}\ndeadline_kind: none\ndeadline: an alarm\n---\n\n\
             # Trigger\n\nX.\n\n# State\n\nY.\n\n# Details\n\n{details}\n"
        )
    };
    // Each case breaks one rule of a clean repository: (the check that must name it, what to write)
    let cases: [(&str, &str, String); 11] = [
        ("the bundle is seen", "docs/backlog/rules.md", String::new()),
        (
            "every backlog document keeps the format",
            "docs/backlog/x.md",
            "# no frontmatter\n".into(),
        ),
        (
            "every link in # Details resolves",
            "docs/backlog/x.md",
            item("[gone](/no_such.md)"),
        ),
        (
            "the log points only at real backlog items",
            "docs/log.md",
            "# Log\n\n## 2026-10-02\n\n- docs/backlog/no-such-item.md\n".into(),
        ),
        (
            "the log keeps its structure",
            "docs/log.md",
            "# Log\n\n## 2026-10-01\n\n## 2026-10-02\n".into(),
        ),
        (
            "the log keeps its structure",
            "docs/log.md",
            "# Log\n\nNo headings, just text.\n".into(),
        ),
        (
            "every document is a known type in its place",
            "docs/other/x.md",
            "---\ntype: Spec\n---\n".into(),
        ),
        (
            "every spec keeps the format",
            "docs/done/x.md",
            "---\ntype: Spec\ntitle: A\ndescription: B.\nstatus: stable\n---\n".into(),
        ),
        (
            "every index is up to date",
            "docs/backlog/index.md",
            "edited by hand\n".into(),
        ),
        (
            "no spec sits at the repository root",
            "genre_spec.md",
            "# spec\n".into(),
        ),
        ("the log keeps its structure", "docs/log.md", String::new()),
    ];
    for (check, path, text) in cases {
        let root = clean_repo();
        let target = root.path().join(path);
        if text.is_empty() {
            fs::remove_file(&target).unwrap();
        } else {
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, text).unwrap();
        }
        let out = run(&["--root", &root_arg(root.path()), "check"]);
        assert_eq!(out.status.code(), Some(1), "{check}: {}", stdout(&out));
        assert!(
            stdout(&out).contains(&format!("{check}:")),
            "{check} did not name it: {}",
            stdout(&out)
        );
    }
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
