//! `docket check`: every rule of the records, run against one repository.
//!
//! - **The backlog works as a backlog**: every document keeps the format, every link in `# Details` resolves, and every
//!   backlog item the log points to exists.
//! - **`docs/` is one OKF bundle**: every document is a known type in the directory for its type, a spec sits in the
//!   directory for its status, every index file equals what `docket index` writes, and no spec sits at the root.
//! - **The log keeps the OKF log structure**: every second-level heading is a date, newest first, and the entries
//!   are a flat list of list items under those dates.
//!
//! Each check has a floor: when the scan finds nothing at all, it fails instead of passing with nothing checked.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::NaiveDate;
use percent_encoding::percent_decode_str;
use regex::Regex;

use crate::bundle::{Bundle, Docs, RESERVED, backlog, specs};
use crate::frontmatter::split;
use crate::markdown::{broken, heading, links, visible};
use crate::schema::SPEC_FOLDERS;
use crate::source::{read_source, relative_path};

/// Directory (relative to docs/, "" for the root) -> the document types allowed in it
const TYPES: [(&str, &[&str]); 4] = [
    ("", &["Guide"]),
    ("backlog", &["Backlog Item", "Guide"]),
    ("specs", &["Spec"]),
    ("done", &["Spec"]),
];

// How the log points to a backlog item. Matched without `docs/`, so pointers written while the backlog was at the
// repository root (`backlog/<slug>.md`) still match an item by its slug. A pointer written with Windows separators
// (`docs\backlog\<slug>.md`) is a pointer too, and has to name an item that exists. So is a link that percent-encodes
// the slug (`backlog/%E6%97%A5.md`). A path without `.md` is not taken for a pointer: in prose, `backlog/` is also
// followed by words that name no file
static BACKLOG_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"backlog[/\\]([\w.%-]+\.md)").unwrap());
// A file at the repository root with one of these names is taken for a spec
static ROOT_SPEC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(spec|仕様).*\.md$").unwrap());
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
// A bullet list item, indented by up to 3 spaces as GFM allows
static LIST_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}[*+-](?:[ \t]|$)").unwrap());

/// One broken rule: which check found it, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub detail: String,
}

/// Every broken rule in the repository at `root`. An error is a file that could not be read at all.
pub fn check(root: &Path) -> io::Result<Vec<Finding>> {
    let bundle = Bundle::new(root);
    let mut found = Vec::new();
    let mut add = |check: &'static str, details: Vec<String>| {
        found.extend(details.into_iter().map(|detail| Finding { check, detail }));
    };

    // The floor: the directories and the root index exist, and the backlog rules are found as a document. If a move
    // or a rename makes the scan come back empty, the checks below see nothing and pass
    let missing: Vec<String> = TYPES
        .iter()
        .map(|(folder, _)| bundle.docs.join(folder))
        .chain([
            bundle.docs.join("index.md"),
            bundle.docs.join("backlog").join("rules.md"),
        ])
        .filter(|path| !path.exists())
        .map(|path| format!("missing: {}", relative_path(&path, root)))
        .collect();
    if !missing.is_empty() {
        add("the bundle is seen", missing);
        return Ok(found);
    }

    let docs = bundle.read_folder("backlog")?;
    let parsed = backlog(&docs);
    add(
        "every backlog document keeps the format",
        pairs(&parsed.problems),
    );
    let details: BTreeMap<String, String> = parsed
        .items
        .iter()
        .map(|(name, (_, sections))| (name.clone(), sections["Details"].clone()))
        .collect();
    add(
        "every link in # Details resolves",
        pairs(&unresolved(&details, root)),
    );

    let log_path = bundle.docs.join("log.md");
    if log_path.is_file() {
        let log = read_source(&log_path)?;
        let names = file_names(&bundle.docs.join("backlog"))?;
        let dangling = dangling_backlog_refs(&log, &names);
        add(
            "the log points only at real backlog items",
            dangling
                .into_iter()
                .map(|name| format!("no such item: docs/backlog/{name}"))
                .collect(),
        );
        add("the log keeps its structure", log_problems(&log));
    } else {
        add(
            "the log keeps its structure",
            vec!["missing: docs/log.md".into()],
        );
    }

    let mut out_of_place = misplaced(&concepts(&bundle.docs)?);
    out_of_place.extend(unread(&bundle.docs)?);
    add(
        "every document is a known type in its place",
        pairs(&out_of_place),
    );
    for (folder, _) in SPEC_FOLDERS {
        let (_, bad) = specs(folder, &bundle.read_folder(folder)?);
        add(
            "every spec keeps the format",
            bad.into_iter()
                .map(|(name, why)| format!("{folder}/{name}: {why}"))
                .collect(),
        );
    }
    let (files, _) = bundle.expected()?;
    let stale: Vec<String> = files
        .into_iter()
        .filter(|(path, text)| read_source(path).ok().as_ref() != Some(text))
        .map(|(path, _)| {
            format!(
                "out of date, run `docket index`: {}",
                relative_path(&path, root)
            )
        })
        .collect();
    add("every index is up to date", stale);
    let names = file_names(root)?;
    add(
        "no spec sits at the repository root",
        root_specs(&names)
            .into_iter()
            .map(|name| format!("specs go in docs/specs/ or docs/done/: {name}"))
            .collect(),
    );
    Ok(found)
}

/// Every name in a directory: files, directories and the rest.
fn file_names(dir: &Path) -> io::Result<Vec<String>> {
    fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect()
}

fn pairs(problems: &BTreeMap<String, String>) -> Vec<String> {
    problems
        .iter()
        .map(|(name, why)| format!("{name}: {why}"))
        .collect()
}

/// Item -> why, for the Details sections with no link or with a link that does not resolve. `root` is the root of the
/// repository.
///
/// Details sit in a backlog item (`docs/backlog/<slug>.md`), so relative links resolve from there and links starting
/// with `/` from the bundle root (`docs/`).
pub fn unresolved(details: &BTreeMap<String, String>, root: &Path) -> BTreeMap<String, String> {
    let bundle_root = root.join("docs");
    let here = bundle_root.join("backlog");
    let mut bad = BTreeMap::new();
    for (name, detail) in details {
        let found = links(detail);
        if found.is_empty() {
            bad.insert(name.clone(), format!("no markdown link: {detail}"));
            continue;
        }
        let reasons: Vec<String> = found
            .iter()
            .filter_map(|(text, target)| broken(text, target, &here, &bundle_root))
            .collect();
        if !reasons.is_empty() {
            bad.insert(
                name.clone(),
                format!("links that do not resolve: {reasons:?}"),
            );
        }
    }
    bad
}

/// The `docs/backlog/<slug>.md` the log points to that are not among `names` (the file names in `docs/backlog/`).
pub fn dangling_backlog_refs(log: &str, names: &[String]) -> Vec<String> {
    let mut dangling: Vec<String> = BACKLOG_REF
        .captures_iter(log)
        .map(|caps| {
            percent_decode_str(&caps[1])
                .decode_utf8_lossy()
                .into_owned()
        })
        .filter(|name| !names.contains(name))
        .collect();
    dangling.sort();
    dangling.dedup();
    dangling
}

/// What breaks the log structure, in reading order.
///
/// Every visible line below the title is a date heading or part of a list item under one. A new log holds only its
/// title and HTML comments, or nothing, and passes. Anything else visible is checked as an entry, so text alone, or
/// dates at another heading level, fails instead of passing with no date checked (the floor).
pub fn log_problems(text: &str) -> Vec<String> {
    let shown = visible(text);
    let mut found = Vec::new();
    let mut last: Option<NaiveDate> = None;
    let dates = shown
        .lines()
        .filter_map(heading)
        .filter(|(level, _)| *level == 2);
    for (_, heading) in dates {
        let day = DATE
            .is_match(heading)
            .then(|| NaiveDate::parse_from_str(heading, "%Y-%m-%d").ok())
            .flatten();
        let Some(day) = day else {
            found.push(format!("not a YYYY-MM-DD date heading: ## {heading}"));
            continue;
        };
        if let Some(previous) = last
            && day >= previous
        {
            found.push(format!(
                "not newest first: ## {heading} comes after ## {previous}"
            ));
        }
        last = Some(day);
    }
    let mut lines = shown
        .lines()
        .filter(|line| !line.trim().is_empty())
        .peekable();
    // The title: a first-level heading on the first line, unless it is a date (an entry one level up)
    lines.next_if(|line| {
        heading(line).is_some_and(|(level, title)| level == 1 && !DATE.is_match(title))
    });
    // OKF 0.2 (section 9): a flat list of entries grouped under the date headings. An entry is a list item; its
    // indented lines (wrapped text, nested items) belong to it. Anything else, a `### <task>` heading included, is not
    // an entry
    let (mut in_group, mut in_item) = (false, false);
    for line in lines {
        if let Some((level, _)) = heading(line) {
            in_item = false;
            if level == 2 {
                in_group = true;
            } else {
                found.push(format!(
                    "a heading other than ## YYYY-MM-DD in the log (entries are list items): {}",
                    line.trim()
                ));
            }
        } else if LIST_ITEM.is_match(line) {
            in_item = true;
            if !in_group {
                found.push(format!(
                    "an entry outside a ## YYYY-MM-DD group in the log: {}",
                    line.trim()
                ));
            }
        } else if !(in_item && line.starts_with([' ', '\t'])) {
            found.push(format!(
                "not a list entry under a date in the log: {}",
                line.trim()
            ));
        }
    }
    found
}

/// Every document under `docs/`: path relative to `docs/` (with `/`) -> text.
pub fn concepts(docs: &Path) -> io::Result<Docs> {
    let mut out = Docs::new();
    for (path, full) in files(docs)? {
        let name = file_name(&path);
        if name.ends_with(".md") && !RESERVED.contains(&name) {
            out.insert(path, read_source(&full)?);
        }
    }
    Ok(out)
}

/// Path -> why, for the files under `docs/` that a reader takes for part of the bundle and docket would not read.
///
/// A reserved name (OKF 0.2, section 3.1) is read only where docket writes or reads it: an `index.md` in a directory
/// that holds documents, and `log.md` at the root. Anywhere else, OKF says it follows the structure of an index or a
/// log, and nothing would check that. A markdown file whose extension is not `.md` in lowercase (`.MD`) is shown by
/// GitHub, but not read as a document, so a broken one would pass.
pub fn unread(docs: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut bad = BTreeMap::new();
    for (path, _) in files(docs)? {
        let name = file_name(&path);
        let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
        let read = match name {
            "index.md" => TYPES.iter().any(|(known, _)| *known == folder),
            "log.md" => folder.is_empty(),
            _ => true,
        };
        if !read {
            bad.insert(
                path,
                "a reserved name outside the places docket writes and reads (index.md in docs/ and in each directory \
                 of documents, log.md in docs/)"
                    .into(),
            );
        } else if !name.ends_with(".md")
            && name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md"))
        {
            bad.insert(
                path,
                "a markdown file is named with .md, in lowercase".into(),
            );
        }
    }
    Ok(bad)
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// Every file under `dir`: path relative to `dir` (with `/`) -> full path.
fn files(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    Ok(out)
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            walk(&entry.path(), &format!("{path}/"), out)?;
        } else {
            out.push((path, entry.path()));
        }
    }
    Ok(())
}

/// Path -> why the document is not a known type in its directory.
pub fn misplaced(docs: &Docs) -> BTreeMap<String, String> {
    let mut bad = BTreeMap::new();
    for (path, text) in docs {
        let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
        let kind = match split(text) {
            Ok((meta, _)) => match meta.get(&yaml_rust2::Yaml::String("type".into())) {
                Some(yaml_rust2::Yaml::String(kind)) => Some(kind.clone()),
                _ => None,
            },
            Err(why) => {
                bad.insert(path.clone(), why);
                continue;
            }
        };
        match TYPES.iter().find(|(name, _)| *name == folder) {
            None => {
                bad.insert(
                    path.clone(),
                    format!("no document belongs in docs/{folder}/"),
                );
            }
            Some((_, allowed)) if !kind.as_deref().is_some_and(|kind| allowed.contains(&kind)) => {
                let shown = if folder.is_empty() { "." } else { folder };
                bad.insert(
                    path.clone(),
                    format!(
                        "type {kind:?} does not belong in docs/{shown} ({})",
                        allowed.join(", ")
                    ),
                );
            }
            Some(_) => {}
        }
    }
    bad
}

/// The names that look like a spec, among the files at the repository root.
pub fn root_specs(names: &[String]) -> Vec<String> {
    let mut found: Vec<String> = names
        .iter()
        .filter(|name| ROOT_SPEC.is_match(name))
        .cloned()
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_details_section_without_a_link_is_caught() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("docs/backlog")).unwrap();
        fs::write(root.path().join("docs/log.md"), "# Log\n").unwrap();
        let details = map(&[
            ("linked.md", "[log](/log.md)"),
            ("no link (the old form).md", "docs/log.md「somewhere」"),
            (
                "only the second link is broken.md",
                "[a](/log.md), [b](/no_such_file.md)",
            ),
        ]);
        let bad = unresolved(&details, root.path());
        assert_eq!(
            bad.keys().collect::<Vec<_>>(),
            [
                "no link (the old form).md",
                "only the second link is broken.md"
            ]
        );
    }

    #[test]
    fn a_dangling_log_pointer_is_caught() {
        let log = "## 2026-10-01\n\n### Something\n- **Open items**: docs/backlog/rules.md, \
                   docs/backlog/no-such-item.md, backlog/rules.md, docs\\backlog\\rules.md, \
                   docs\\backlog\\written-on-windows.md, [日](/backlog/%E6%97%A5.md), \
                   [月](/backlog/%E6%9C%88.md)\n";
        let names = ["rules.md", "index.md", "日.md"].map(String::from).to_vec();
        // A link to a slug outside ASCII is percent-encoded, and points at the item all the same
        assert_eq!(
            dangling_backlog_refs(log, &names),
            ["no-such-item.md", "written-on-windows.md", "月.md"]
        );
    }

    #[test]
    fn the_log_structure_is_checked() {
        let good = [
            (
                "entries",
                "# Log\n\n<!--\n## YYYY-MM-DD\n-->\n\n## 2026-10-02\n\n* **b**\n  * **Branch**: main\n    wrapped\n\n## 2026-10-01\n\n- a\n\n  more of a\n\n```\n## x\n```\n",
            ),
            // A new repository has no entries yet: the title and the format guide, or nothing at all
            (
                "only the title and the guide",
                "# Log\n\n<!--\n### <Task name>\n-->\n",
            ),
            ("empty", ""),
            // GitHub shows these as the same heading
            (
                "a date heading with spaces around it",
                "  ## 2026-10-02 \n\n## 2026-10-01 ##\n",
            ),
            ("an indented title", " # Log\n"),
        ];
        for (name, text) in good {
            assert_eq!(log_problems(text), Vec::<String>::new(), "{name}");
        }
        let bad = [
            ("task on the date line", "## 2026-10-01 a task\n"),
            ("not a real date", "## 2026-13-01\n"),
            ("oldest first", "## 2026-10-01\n\n## 2026-10-02\n"),
            ("same date twice", "## 2026-10-01\n\n## 2026-10-01\n"),
            ("digits of another script", "## ２０２６-10-01\n"),
            // The floor: text under the title means entries, and entries sit under a date
            ("text without a date", "# Log\n\nNo headings, just text.\n"),
            ("dates one level up", "# Log\n\n# 2026-10-02\n\n* a\n"),
            // OKF 0.2 (section 9): entries are a flat list under the dates, not sections
            (
                "a task heading",
                "## 2026-10-02\n\n### a task\n\n- **Branch**: main\n",
            ),
            (
                "a paragraph under a date",
                "## 2026-10-02\n\nDid something.\n",
            ),
            (
                "an entry before the first date",
                "# Log\n\n* early\n\n## 2026-10-02\n\n* a\n",
            ),
            ("dates one level down", "# Log\n\n### 2026-10-02\n"),
            ("a date for the title", "# 2026-10-02\n"),
            // GFM lets a heading be indented by up to 3 spaces
            (
                "an indented date heading out of order",
                "## 2026-10-01\n\n  ## 2026-10-02\n",
            ),
            (
                "an indented heading that is not a date",
                "## 2026-10-02\n\n   ## not a date\n",
            ),
        ];
        let passed: Vec<&str> = bad
            .iter()
            .filter(|(_, text)| log_problems(text).is_empty())
            .map(|(name, _)| *name)
            .collect();
        assert!(passed.is_empty(), "passed: {passed:?}");
    }

    const SPEC: &str = "---\ntype: Spec\ntitle: Something\ndescription: One sentence.\nstatus: stable\n---\n\n# Goals\n";

    #[test]
    fn a_known_type_in_its_place_passes() {
        let docs = map(&[
            ("specs/good.md", SPEC),
            ("backlog/rules.md", "---\ntype: Guide\n---\n"),
            ("guide.md", "---\ntype: Guide\n---\n"),
        ]);
        assert_eq!(misplaced(&docs), BTreeMap::new());
    }

    #[test]
    fn a_misplaced_document_is_caught() {
        let item = "---\ntype: Backlog Item\n---\n";
        let bad = map(&[
            ("specs/item.md", item),
            ("item.md", item),
            ("other/x.md", SPEC),
            ("backlog/plain.md", "# Just markdown\n"),
            ("backlog/no-type.md", "---\ntitle: x\n---\n"),
            ("specs/deeper/x.md", SPEC),
        ]);
        let found = misplaced(&bad);
        assert_eq!(
            found.keys().collect::<Vec<_>>(),
            bad.keys().collect::<Vec<_>>()
        );
    }

    /// A `docs/` with the given files, each empty.
    fn docs_with(paths: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for path in paths {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, "").unwrap();
        }
        root
    }

    #[test]
    fn the_files_docket_reads_pass() {
        let docs = docs_with(&[
            "index.md",
            "log.md",
            "backlog/index.md",
            "backlog/item.md",
            "specs/index.md",
            "done/index.md",
            "assets/diagram.png",
        ]);
        assert_eq!(unread(docs.path()).unwrap(), BTreeMap::new());
    }

    #[test]
    fn a_file_docket_would_not_read_is_caught() {
        let bad = [
            // Reserved names where docket neither writes nor reads them
            "extra/index.md",
            "specs/deeper/index.md",
            "backlog/log.md",
            // Markdown that is not named .md
            "backlog/item.MD",
            "notes.Md",
        ];
        let docs = docs_with(&bad);
        let found = unread(docs.path()).unwrap();
        assert_eq!(found.keys().collect::<Vec<_>>(), {
            let mut sorted = bad.to_vec();
            sorted.sort();
            sorted
        });
    }

    #[test]
    fn a_root_spec_is_caught() {
        let names: Vec<String> = [
            "README.md",
            "AGENTS.md",
            "genre_spec.md",
            "仕様書.md",
            "pyproject.toml",
            "SPEC.MD",
        ]
        .map(String::from)
        .into();
        assert_eq!(
            root_specs(&names),
            ["SPEC.MD", "genre_spec.md", "仕様書.md"]
        );
    }
}
