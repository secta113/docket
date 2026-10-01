//! Checks that what the repository says about itself agrees with the repository. Both drifted silently before: the map
//! in `AGENTS.md` once named 6 of the tracked top-level paths, and the toolchain version is written in three files.

use std::collections::BTreeSet;

/// Fewer table rows than this means the map's format changed and nothing was read, not that the repository shrank
pub const MIN_ROWS: usize = 10;

/// The map in `AGENTS.md` (the `## Map` section).
#[derive(Debug, Default)]
pub struct Map {
    /// Top-level paths: the first column of the path table, and the names in the prose under it
    pub paths: BTreeSet<String>,
    /// Modules of `src/`: the first column of the table whose header names `src/`
    pub modules: BTreeSet<String>,
    /// Table rows read, headers and separators excluded
    pub rows: usize,
}

fn backticked(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split('`')
        .skip(1)
        .step_by(2)
        .map(|n| n.trim_end_matches('/').to_string())
}

pub fn read_map(agents_md: &str) -> Map {
    let mut map = Map::default();
    let (mut in_map, mut header, mut modules_table) = (false, true, false);
    for line in agents_md.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_map = heading.trim() == "Map";
            continue;
        }
        if !in_map {
            continue;
        }
        let Some(row) = line.trim_start().strip_prefix('|') else {
            header = true;
            map.paths.extend(backticked(line));
            continue;
        };
        let first = row.split('|').next().unwrap_or_default();
        if first.trim().starts_with("---") {
            continue;
        }
        if header {
            modules_table = backticked(first).any(|n| n == "src");
            header = false;
            continue;
        }
        map.rows += 1;
        if modules_table {
            map.modules.extend(backticked(first));
        } else {
            map.paths.extend(backticked(first));
        }
    }
    map
}

/// Where the map and the tracked files (`git ls-files`, `/`-separated) disagree.
pub fn map_problems(map: &Map, tracked: &[String]) -> Vec<String> {
    let top: BTreeSet<String> = tracked
        .iter()
        .filter_map(|f| f.split('/').next())
        .map(str::to_string)
        .collect();
    let modules: BTreeSet<String> = tracked
        .iter()
        .filter_map(|f| f.strip_prefix("src/"))
        .filter(|f| !f.contains('/') && f.ends_with(".rs"))
        .map(str::to_string)
        .collect();
    let mut found = Vec::new();
    if map.rows < MIN_ROWS {
        found.push(format!(
            "the map in AGENTS.md has {} table rows, fewer than {MIN_ROWS}: did its format change?",
            map.rows
        ));
    }
    for p in top.difference(&map.paths) {
        found.push(format!("{p} is tracked but not in the map in AGENTS.md"));
    }
    for p in map.paths.difference(&top) {
        found.push(format!("{p} is in the map in AGENTS.md but not tracked"));
    }
    for m in modules.difference(&map.modules) {
        found.push(format!(
            "src/{m} is tracked but not in the module table in AGENTS.md"
        ));
    }
    for m in map.modules.difference(&modules) {
        found.push(format!(
            "src/{m} is in the module table in AGENTS.md but not tracked"
        ));
    }
    found
}

/// Where the toolchain version differs between `rust-toolchain.toml`, the base image of the `Dockerfile` and the
/// container of the CI workflow. A file where the version cannot be found is a problem too: the check would otherwise
/// pass with nothing compared.
pub fn toolchain_problems(toolchain: &str, dockerfile: &str, ci: &str) -> Vec<String> {
    let Some(channel) = toolchain.lines().find_map(|l| {
        let (key, value) = l.split_once('=')?;
        (key.trim() == "channel").then(|| value.trim().trim_matches('"').to_string())
    }) else {
        return vec!["rust-toolchain.toml names no channel".into()];
    };
    let images = |text: &str, prefix: &str| -> Vec<String> {
        text.lines()
            .filter_map(|l| l.trim().strip_prefix(prefix))
            .filter_map(|rest| rest.split_whitespace().next())
            .map(str::to_string)
            .collect()
    };
    let mut found = Vec::new();
    for (file, versions) in [
        ("Dockerfile", images(dockerfile, "FROM rust:")),
        (".github/workflows/ci.yml", images(ci, "container: rust:")),
    ] {
        if versions.is_empty() {
            found.push(format!("{file} names no rust image"));
        }
        for v in versions.iter().filter(|v| **v != channel) {
            found.push(format!(
                "{file} uses rust:{v}, but rust-toolchain.toml names {channel}"
            ));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENTS: &str = "\
# AGENTS.md

## Map

| Path | Content |
|---|---|
| `src/` | The tool |
| `a.toml`, `b.toml` | Two files |

Also tracked: `c`.

| Module of `src/` | Content |
|---|---|
| `main.rs` | The command line |
| `lib.rs` | The library |

## Rules

- `not-a-path` is outside the map
";

    fn tracked(files: &[&str]) -> Vec<String> {
        files.iter().map(|f| f.to_string()).collect()
    }

    fn problems(agents: &str, files: &[&str]) -> Vec<String> {
        let map = read_map(agents);
        map_problems(&map, &tracked(files))
            .into_iter()
            .filter(|p| !p.contains("table rows"))
            .collect()
    }

    const TREE: &[&str] = &["src/main.rs", "src/lib.rs", "a.toml", "b.toml", "c"];

    #[test]
    fn reads_both_tables_and_the_prose_of_the_map_only() {
        let map = read_map(AGENTS);
        assert_eq!(
            map.paths,
            ["a.toml", "b.toml", "c", "src"].map(String::from).into()
        );
        assert_eq!(map.modules, ["lib.rs", "main.rs"].map(String::from).into());
        assert_eq!(map.rows, 4);
    }

    #[test]
    fn a_map_that_matches_the_tree_passes() {
        assert_eq!(problems(AGENTS, TREE), Vec::<String>::new());
    }

    #[test]
    fn a_tracked_path_missing_from_the_map_fails() {
        let found = problems(AGENTS, &[TREE, &["Dockerfile"]].concat());
        assert_eq!(
            found,
            ["Dockerfile is tracked but not in the map in AGENTS.md"]
        );
    }

    #[test]
    fn a_row_for_a_path_that_is_gone_fails() {
        let found = problems(AGENTS, &["src/main.rs", "src/lib.rs", "a.toml", "c"]);
        assert_eq!(found, ["b.toml is in the map in AGENTS.md but not tracked"]);
    }

    #[test]
    fn a_module_missing_from_the_table_fails_and_so_does_a_row_for_a_gone_module() {
        let found = problems(
            AGENTS,
            &["src/main.rs", "src/new.rs", "a.toml", "b.toml", "c"],
        );
        assert_eq!(
            found,
            [
                "src/new.rs is tracked but not in the module table in AGENTS.md",
                "src/lib.rs is in the module table in AGENTS.md but not tracked",
            ]
        );
    }

    #[test]
    fn a_map_read_as_empty_fails_by_its_floor() {
        let map = read_map("# AGENTS.md\n\nNo map here.\n");
        let found = map_problems(&map, &tracked(TREE));
        assert!(found[0].contains("0 table rows"), "{found:?}");
    }

    const TOOLCHAIN: &str = "[toolchain]\nchannel = \"1.98.1\"\n";

    #[test]
    fn the_same_version_everywhere_passes() {
        let found = toolchain_problems(
            TOOLCHAIN,
            "FROM rust:1.98.1\n",
            "    container: rust:1.98.1\n",
        );
        assert_eq!(found, Vec::<String>::new());
    }

    #[test]
    fn a_different_version_in_the_dockerfile_or_the_workflow_fails() {
        let found = toolchain_problems(
            TOOLCHAIN,
            "FROM rust:1.97.0 AS build\n",
            "    container: rust:1.99.0\n",
        );
        assert_eq!(
            found,
            [
                "Dockerfile uses rust:1.97.0, but rust-toolchain.toml names 1.98.1",
                ".github/workflows/ci.yml uses rust:1.99.0, but rust-toolchain.toml names 1.98.1",
            ]
        );
    }

    #[test]
    fn a_version_that_cannot_be_found_fails() {
        assert_eq!(
            toolchain_problems("[toolchain]\n", "", ""),
            ["rust-toolchain.toml names no channel"]
        );
        assert_eq!(
            toolchain_problems(TOOLCHAIN, "FROM debian\n", "runs-on: ubuntu-latest\n"),
            [
                "Dockerfile names no rust image",
                ".github/workflows/ci.yml names no rust image",
            ]
        );
    }
}
