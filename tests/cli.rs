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

/// A repository of records only with an empty bundle: the directories of `docs/`, nothing in them.
fn repo() -> tempfile::TempDir {
    let root = declared("stack = \"none\"\nareas = [\"operations\"]\n");
    let docs = root.path().join("docs");
    for folder in ["backlog", "specs", "done"] {
        fs::create_dir_all(docs.join(folder)).unwrap();
    }
    root
}

/// A repository with only `.config/docket.toml`.
fn declared(declaration: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".config")).unwrap();
    fs::write(root.path().join(".config/docket.toml"), declaration).unwrap();
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

/// A repository that keeps every rule: what `docket create` makes for a Python project without `ui`, and a log entry.
fn clean_repo() -> tempfile::TempDir {
    let root = declared("stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\n");
    let out = run(&["--root", &root_arg(root.path()), "create"]);
    assert!(out.status.success(), "{}", stdout(&out));
    fs::write(
        root.path().join("docs/log.md"),
        "# Log\n\n## 2026-10-02\n\n* Something\n",
    )
    .unwrap();
    root
}

#[test]
fn a_clean_repository_passes() {
    let root = clean_repo();
    let out = run(&["--root", &root_arg(root.path()), "check"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("the layers and every record keep the rules"));
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
    let cases: [(&str, &str, String); 18] = [
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
        // The only link is inside a comment, so a reader sees none
        (
            "every link in # Details resolves",
            "docs/backlog/x.md",
            item("Nothing yet. <!-- [log](/log.md) -->"),
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
            "every generated file is up to date",
            "docs/backlog/index.md",
            "edited by hand\n".into(),
        ),
        // The rules a project reads are the rules its docket checks
        (
            "every generated file is up to date",
            "docs/backlog/rules.md",
            "---\ntype: Guide\ntitle: Backlog rules\ndescription: Our own.\n---\n".into(),
        ),
        (
            "no spec sits at the repository root",
            "genre_spec.md",
            "# spec\n".into(),
        ),
        ("the log keeps its structure", "docs/log.md", String::new()),
        // The area of a record is declared, and the declared areas are distinct headings
        (
            "every backlog document keeps the format",
            "docs/backlog/x.md",
            item("[log](/log.md)").replace("tags: [a]", "tags: [b]"),
        ),
        (
            "every spec keeps the format",
            "docs/specs/x.md",
            "---\ntype: Spec\ntitle: A\ndescription: B.\ntags: [b]\nstatus: stable\n---\n".into(),
        ),
        (
            "every spec keeps the format",
            "docs/specs/x.md",
            "---\ntype: Spec\ntitle: A\ndescription: B.\nstatus: stable\n---\n".into(),
        ),
        (
            "the areas are distinct headings",
            ".config/docket.toml",
            "stack = \"python\"\nareas = [\"a\", \"A\"]\nabsent = [\"ui\"]\n".into(),
        ),
        // Without the areas, no record can be judged: the check says so instead of passing them
        (
            "the bundle is seen",
            ".config/docket.toml",
            "stack = \"python\"\nabsent = [\"ui\"]\n".into(),
        ),
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
fn an_epic_and_its_parts_are_checked_together() {
    let spec = |epic: &str, status: &str| {
        format!(
            "---\ntype: Spec\ntitle: X\ndescription: Y.\ntags: [a]\nstatus: {status}\n{epic}---\n\n\
             # Resolution\n\nDone.\n"
        )
    };
    // (the check that must name it, the epic's directory, the slug the part names)
    let cases = [
        ("an epic closes after its parts", "done", "big"),
        ("every spec keeps the format", "specs", "no-such-spec"),
    ];
    for (check, epic_folder, named) in cases {
        let root = clean_repo();
        let r = root.path();
        let status = if epic_folder == "done" {
            "deprecated"
        } else {
            "stable"
        };
        fs::write(
            r.join(format!("docs/{epic_folder}/big.md")),
            spec("", status),
        )
        .unwrap();
        fs::write(
            r.join("docs/specs/part.md"),
            spec(&format!("epic: {named}\n"), "stable"),
        )
        .unwrap();
        // The index files are current, so only the relation can fail
        assert!(run(&["--root", &root_arg(r), "index"]).status.success());
        let out = run(&["--root", &root_arg(r), "check"]);
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
        "docs/backlog/rules.md",
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
        backlog.contains("* [Backlog rules](rules.md) - What goes in docs/backlog/"),
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
fn index_fails_without_the_areas() {
    let root = repo();
    fs::remove_file(root.path().join(".config/docket.toml")).unwrap();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("docket init --stack"));
    assert!(!root.path().join("docs/index.md").exists());
}

#[test]
fn index_fails_without_a_spec_directory() {
    let root = repo();
    fs::remove_dir(root.path().join("docs/done")).unwrap();
    let out = run(&["--root", &root_arg(root.path()), "index"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("done"));
}

/// Every file under `root`, from the root with `/`, and its text.
fn tree(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let name = path.strip_prefix(root).unwrap().to_string_lossy();
                out.push((name.replace('\\', "/"), fs::read_to_string(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn create_makes_each_stack_once_and_check_passes_on_it() {
    // (stack, files that must be made, paths that must not)
    let stacks: [(&str, &[&str], &[&str]); 3] = [
        (
            "python",
            &[
                "handler/__init__.py",
                "ui/atoms/__init__.py",
                "utils/__init__.py",
            ],
            &[],
        ),
        (
            "typescript",
            &[
                "src/handler/index.ts",
                "src/ui/pages/index.ts",
                "src/domain/index.ts",
            ],
            &[],
        ),
        (
            "rust",
            &[
                "crates/handler/src/main.rs",
                "crates/domain/Cargo.toml",
                "crates/domain/src/lib.rs",
            ],
            &["crates/ui"],
        ),
    ];
    for (stack, made, not_made) in stacks {
        let root = declared(&format!("stack = \"{stack}\"\nareas = [\"a\"]\n"));
        let arg = root_arg(root.path());
        let first = run(&["--root", &arg, "create"]);
        assert!(first.status.success(), "{stack}: {}", stdout(&first));
        for path in made
            .iter()
            .chain(&["docs/log.md", "docs/backlog/rules.md", "docs/index.md"])
        {
            assert!(
                root.path().join(path).is_file(),
                "{stack}: {path} was not made"
            );
            assert!(
                stdout(&first).contains(&format!("wrote {path}")),
                "{stack}: {}",
                stdout(&first)
            );
        }
        for path in not_made {
            assert!(!root.path().join(path).exists(), "{stack}: {path} was made");
        }
        let before = tree(root.path());
        let second = run(&["--root", &arg, "create"]);
        assert!(second.status.success());
        assert!(
            stdout(&second).contains("nothing to make"),
            "{stack}: {}",
            stdout(&second)
        );
        assert_eq!(
            tree(root.path()),
            before,
            "{stack}: the second run changed the tree"
        );
        let check = run(&["--root", &arg, "check"]);
        assert!(check.status.success(), "{stack}: {}", stdout(&check));
    }
}

#[test]
fn create_respects_absent_and_leaves_what_it_does_not_own() {
    let root = declared(
        "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui.templates\", \"infrastructure\"]\n",
    );
    let arg = root_arg(root.path());
    // Files of the project: a layer it already has, and a log with entries
    fs::create_dir_all(root.path().join("domain")).unwrap();
    fs::write(root.path().join("domain/model.py"), "X = 1\n").unwrap();
    fs::create_dir_all(root.path().join("docs")).unwrap();
    let log = "# Log\n\n## 2026-10-02\n\n* Mine\n";
    fs::write(root.path().join("docs/log.md"), log).unwrap();
    let out = run(&["--root", &arg, "create"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(root.path().join("ui/pages/__init__.py").is_file());
    assert!(!root.path().join("ui/templates").exists());
    assert!(!root.path().join("infrastructure").exists());
    // A present layer is the project's: not even its missing __init__.py is written
    assert!(!root.path().join("domain/__init__.py").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("domain/model.py")).unwrap(),
        "X = 1\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("docs/log.md")).unwrap(),
        log
    );
    let check = run(&["--root", &arg, "check"]);
    assert!(check.status.success(), "{}", stdout(&check));
}

#[test]
fn create_fails_without_a_declaration_it_can_read() {
    let cases = [
        (None, "docket init --stack"),
        (
            Some("stack = \"cobol\"\nareas = [\"a\"]\n"),
            "unknown stack",
        ),
        (
            Some("stack = \"rust\"\nareas = [\"a\"]\nabsent = [\"ui\"]\n"),
            "does not have",
        ),
        (
            Some("stack = \"python\"\nareas = [\"a\"]\nabsnet = []\n"),
            "unknown field",
        ),
        // A declaration written before areas existed names the field it lacks
        (Some("stack = \"python\"\n"), "missing field `areas`"),
    ];
    for (declaration, said) in cases {
        let root = match declaration {
            Some(text) => declared(text),
            None => tempfile::tempdir().unwrap(),
        };
        let out = run(&["--root", &root_arg(root.path()), "create"]);
        assert_eq!(out.status.code(), Some(2), "{said}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(said), "{said}: {stderr}");
        assert!(
            !root.path().join("docs").exists(),
            "{said}: made docs/ all the same"
        );
    }
}

type Plant = Box<dyn Fn(&Path)>;

fn declare(root: &Path, text: &str) {
    fs::write(root.join(".config/docket.toml"), text).unwrap();
}

fn plant_file(root: &Path, path: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, "x = 1\n").unwrap();
}

#[test]
fn each_difference_from_the_declaration_fails() {
    // Each case breaks one rule of a clean repository: (what the message says, how to break it)
    let cases: Vec<(&str, Plant)> = vec![
        (
            "domain is missing",
            Box::new(|r| fs::remove_dir_all(r.join("domain")).unwrap()),
        ),
        (
            "ui is declared absent, but ui/ exists",
            Box::new(|r| plant_file(r, "ui/__init__.py")),
        ),
        (
            "code outside the layers: scripts",
            Box::new(|r| plant_file(r, "scripts/tool.py")),
        ),
        (
            "code outside the layers: main.py",
            Box::new(|r| plant_file(r, "main.py")),
        ),
        (
            "missing: .config/docket.toml",
            Box::new(|r| fs::remove_file(r.join(".config/docket.toml")).unwrap()),
        ),
        (
            "unknown stack",
            Box::new(|r| declare(r, "stack = \"cobol\"\nareas = [\"a\"]\n")),
        ),
        (
            "does not have",
            Box::new(|r| {
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\", \"service\"]\n",
                )
            }),
        ),
        (
            "unknown field",
            Box::new(|r| {
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\nextra = 1\n",
                )
            }),
        ),
        // A layer cannot be switched off by listing it, or a directory holding it
        (
            "which holds or sits in the layer domain",
            Box::new(|r| {
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\nunchecked = [\"domain\"]\n",
                )
            }),
        ),
        // Written another way, a path would match nothing and switch nothing off
        (
            "write a path from the root",
            Box::new(|r| {
                plant_file(r, "scripts/tool.py");
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\nunchecked = [\"./scripts\"]\n",
                );
            }),
        ),
        (
            "unchecked lists scripts, which does not exist",
            Box::new(|r| {
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\nunchecked = [\"scripts\"]\n",
                )
            }),
        ),
        // The floor: with every layer declared absent, nothing would be checked
        (
            "no layer is present",
            Box::new(|r| {
                for layer in [
                    "handler",
                    "application",
                    "infrastructure",
                    "domain",
                    "utils",
                ] {
                    fs::remove_dir_all(r.join(layer)).unwrap();
                }
                declare(
                    r,
                    "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"handler\", \"ui\", \"application\", \"infrastructure\", \
                     \"domain\", \"utils\"]\n",
                );
            }),
        ),
    ];
    for (said, plant) in cases {
        let root = clean_repo();
        plant(root.path());
        let out = run(&["--root", &root_arg(root.path()), "check"]);
        assert_eq!(out.status.code(), Some(1), "{said}: {}", stdout(&out));
        assert!(
            stdout(&out).contains("the tree matches .config/docket.toml:"),
            "{said}: {}",
            stdout(&out)
        );
        assert!(stdout(&out).contains(said), "{said}: {}", stdout(&out));
    }
}

#[test]
fn code_that_is_not_the_projects_is_not_looked_at() {
    let root = clean_repo();
    let r = root.path();
    // Ignored by git, hidden, paths the stack says are not layers, a path the project lists, and a file that is not
    // code
    fs::write(r.join(".gitignore"), "venv/\n").unwrap();
    for path in [
        "venv/Lib/site-packages/pkg/__init__.py",
        ".tox/x.py",
        "tests/test_x.py",
        "ci.py",
        "scripts/tool.py",
        "notes/readme.md",
    ] {
        plant_file(r, path);
    }
    declare(
        r,
        "stack = \"python\"\nareas = [\"a\"]\nabsent = [\"ui\"]\nunchecked = [\"scripts/\"]\n",
    );
    let out = run(&["--root", &root_arg(r), "check"]);
    assert!(out.status.success(), "{}", stdout(&out));
}

#[test]
fn only_the_projects_own_gitignore_hides_code() {
    // A .gitignore above the root belongs to another repository, or to no repository at all
    let outer = tempfile::tempdir().unwrap();
    fs::write(outer.path().join(".gitignore"), "scripts/\n").unwrap();
    let r = outer.path().join("project");
    fs::create_dir_all(r.join(".config")).unwrap();
    declare(
        &r,
        "stack = \"typescript\"\nareas = [\"a\"]\nabsent = [\"ui\"]\n",
    );
    assert!(run(&["--root", &root_arg(&r), "create"]).status.success());
    plant_file(&r, "src/scripts/tool.ts");
    let out = run(&["--root", &root_arg(&r), "check"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("code outside the layers: src/scripts"),
        "{}",
        stdout(&out)
    );
    // The root's own .gitignore applies inside the scope
    fs::write(r.join(".gitignore"), "src/scripts/\n").unwrap();
    let out = run(&["--root", &root_arg(&r), "check"]);
    assert!(out.status.success(), "{}", stdout(&out));
}

#[test]
fn init_writes_a_declaration_that_create_reads() {
    for stack in ["python", "typescript", "rust", "none"] {
        let root = tempfile::tempdir().unwrap();
        let arg = root_arg(root.path());
        let out = run(&["--root", &arg, "init", "--stack", stack]);
        assert!(
            out.status.success(),
            "{stack}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stdout(&out).contains("wrote .config/docket.toml"),
            "{stack}: {}",
            stdout(&out)
        );
        let written = fs::read_to_string(root.path().join(".config/docket.toml")).unwrap();
        assert!(
            written.contains(&format!("stack = \"{stack}\"")),
            "{written}"
        );
        // Only the declaration: nothing is made before the project declares what it does not have
        assert_eq!(
            tree(root.path()).len(),
            1,
            "{stack}: {:?}",
            tree(root.path())
        );
        let out = run(&["--root", &arg, "create"]);
        assert!(out.status.success(), "{stack}: {}", stdout(&out));
        let out = run(&["--root", &arg, "check"]);
        assert!(out.status.success(), "{stack}: {}", stdout(&out));
        // The declaration is the project's: a second init leaves it as it is
        declare(root.path(), &format!("{written}# edited\n"));
        let again = run(&["--root", &arg, "init", "--stack", stack]);
        assert_eq!(again.status.code(), Some(2), "{stack}");
        assert!(String::from_utf8_lossy(&again.stderr).contains("never overwrites"));
        let kept = fs::read_to_string(root.path().join(".config/docket.toml")).unwrap();
        assert!(kept.ends_with("# edited\n"), "{stack}: {kept}");
    }
}

#[test]
fn init_needs_a_known_stack() {
    let root = tempfile::tempdir().unwrap();
    let arg = root_arg(root.path());
    let out = run(&["--root", &arg, "init", "--stack", "cobol"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("known: python, typescript, rust, none"));
    let out = run(&["--root", &arg, "init"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(tree(root.path()).is_empty(), "{:?}", tree(root.path()));
}

#[test]
fn a_repository_of_records_only_makes_and_checks_only_docs() {
    let root = declared("stack = \"none\"\nareas = [\"a\"]\n");
    let arg = root_arg(root.path());
    let out = run(&["--root", &arg, "create"]);
    assert!(out.status.success(), "{}", stdout(&out));
    let made: Vec<String> = tree(root.path())
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert!(
        made.iter()
            .all(|path| path.starts_with("docs/") || path.starts_with(".config/")),
        "{made:?}"
    );
    // Code anywhere is not looked at, and the output says the layers were skipped
    plant_file(root.path(), "main.py");
    let out = run(&["--root", &arg, "check"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("the layers are not checked"),
        "{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("every record keeps the rules"),
        "{}",
        stdout(&out)
    );
    // The records are still checked
    fs::write(
        root.path().join("docs/backlog/index.md"),
        "edited by hand\n",
    )
    .unwrap();
    let out = run(&["--root", &arg, "check"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    // No layers means nothing to declare absent or unchecked
    declare(
        root.path(),
        "stack = \"none\"\nareas = [\"a\"]\nabsent = [\"ui\"]\n",
    );
    let out = run(&["--root", &arg, "check"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stdout(&out).contains("absent must be empty"),
        "{}",
        stdout(&out)
    );
}
