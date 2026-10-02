//! `docs/` as one OKF 0.2 bundle: reading its documents, and the index file each directory should contain.
//!
//! Every `.md` under `docs/` except the reserved names is a document with frontmatter. The index files are never
//! written by hand: `docket index` writes them, and `docket check` fails when one differs from what it would write.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::frontmatter::Sections;
use crate::schema::{
    BacklogDoc, CLOSED_SECTION, DeadlineKind, Guide, Item, SPEC_FOLDERS, Spec, Status, Time,
    backlog_doc, spec,
};
use crate::source::read_source;

/// File names OKF reserves. Never used for a document
pub const RESERVED: [&str; 2] = ["index.md", "log.md"];
/// The notice at the top of every generated index. An HTML comment, so OKF readers do not see it
pub const GENERATED: &str = "<!-- Generated from the frontmatter by `docket index`. Do not edit: `docket check` fails \
                             when this file differs from what `docket index` writes. -->";
/// The backlog rules. docket writes them like an index file, so the rules a project reads are the rules its docket
/// checks
pub const RULES: &str = include_str!("../records/rules.md");
/// The log as `docket create` makes it. From then on it is the project's
pub const LOG: &str = include_str!("../records/log.md");
/// The bundle-root index links to these, in this order
const ROOT_ENTRIES: [(&str, &str, &str); 4] = [
    ("Backlog", "backlog/", "Open problems and postponed work."),
    ("Specs", "specs/", "Specs being written or in progress."),
    ("Done", "done/", "Closed specs: implemented or dropped."),
    ("Log", "log.md", "What was done, newest first."),
];

/// File name -> text.
pub type Docs = BTreeMap<String, String>;
/// File name -> why it was left out.
pub type Problems = BTreeMap<String, String>;

/// The documents of `docs/backlog/`, sorted out.
#[derive(Debug, Default)]
pub struct Backlog {
    pub items: BTreeMap<String, (Item, Sections)>,
    pub guides: BTreeMap<String, Guide>,
    pub problems: Problems,
}

pub fn backlog(docs: &Docs) -> Backlog {
    let mut out = Backlog::default();
    for (name, text) in docs {
        match backlog_doc(text) {
            Ok(BacklogDoc::Item(item, sections)) => {
                out.items.insert(name.clone(), (item, sections));
            }
            Ok(BacklogDoc::Guide(guide)) => {
                out.guides.insert(name.clone(), guide);
            }
            Err(why) => {
                out.problems.insert(name.clone(), why);
            }
        }
    }
    out
}

/// The specs of `docs/<folder>/` that pass, and why the others do not.
pub fn specs(folder: &str, docs: &Docs) -> (BTreeMap<String, (Spec, Sections)>, Problems) {
    let mut passed = BTreeMap::new();
    let mut problems = Problems::new();
    for (name, text) in docs {
        match spec(folder, text) {
            Ok(parsed) => {
                passed.insert(name.clone(), parsed);
            }
            Err(why) => {
                problems.insert(name.clone(), why);
            }
        }
    }
    (passed, problems)
}

/// The bundle of one repository: `<root>/docs`.
pub struct Bundle {
    pub docs: PathBuf,
}

impl Bundle {
    pub fn new(root: &Path) -> Self {
        Bundle {
            docs: root.join("docs"),
        }
    }

    /// The documents directly in `docs/<folder>/`, except reserved names: file name -> text.
    pub fn read_folder(&self, folder: &str) -> io::Result<Docs> {
        let path = self.docs.join(folder);
        let mut docs = Docs::new();
        for entry in fs::read_dir(&path).map_err(|e| with_path(e, &path))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".md")
                && !RESERVED.contains(&name.as_str())
                && entry.file_type()?.is_file()
            {
                let text = read_source(&entry.path()).map_err(|e| with_path(e, &entry.path()))?;
                docs.insert(name, text);
            }
        }
        Ok(docs)
    }

    /// Every file docket generates in the bundle (the index files and the backlog rules) -> what it should contain
    /// now, and the documents left out of the index files.
    pub fn expected(&self) -> io::Result<(Vec<(PathBuf, String)>, Problems)> {
        // The rules as they are about to be written, so the backlog index lists them on the run that writes them
        let mut docs = self.read_folder("backlog")?;
        docs.insert("rules.md".into(), RULES.into());
        let backlog = backlog(&docs);
        let mut files = vec![
            (self.docs.join("backlog").join("rules.md"), RULES.into()),
            (self.docs.join("index.md"), render_root()),
            (
                self.docs.join("backlog").join("index.md"),
                render_backlog(&backlog.items, &backlog.guides),
            ),
        ];
        let mut problems = backlog.problems;
        for (folder, _) in SPEC_FOLDERS {
            let (passed, bad) = specs(folder, &self.read_folder(folder)?);
            files.push((
                self.docs.join(folder).join("index.md"),
                render_specs(folder, &passed),
            ));
            problems.extend(
                bad.into_iter()
                    .map(|(name, why)| (format!("{folder}/{name}"), why)),
            );
        }
        Ok((files, problems))
    }
}

fn with_path(e: io::Error, path: &Path) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// The first sentence of the first line, for the index. Bold text at the start counts as a sentence on its own.
/// Works for any language: a sentence ends at "。" or at "." followed by a space or the end of the line.
pub fn first_sentence(text: &str) -> &str {
    let first = text.split('\n').next().unwrap_or("").trim();
    if let Some(rest) = first.strip_prefix("**")
        && let Some(end) = rest.find('*')
        && end > 0
        && rest[end..].starts_with("**")
    {
        return &first[..end + 4];
    }
    let mut chars = first.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let ends = match c {
            '。' => true,
            '.' => chars.peek().is_none_or(|(_, next)| next.is_whitespace()),
            _ => false,
        };
        if ends {
            return &first[..i + c.len_utf8()];
        }
    }
    first
}

/// The backlog index (an OKF index.md): the guides, the open items by area, and the closed items last so they do not
/// bury the open ones.
///
/// Each entry has the OKF form `* [title](target) - description`, with the frontmatter's `description`. An open item
/// adds, after ` | `, the date of the last measurement, the first sentence of its state and its deadline: what a
/// reader of the backlog needs. The separator is a symbol so the parts stay apart in any language.
pub fn render_backlog(
    items: &BTreeMap<String, (Item, Sections)>,
    guides: &BTreeMap<String, Guide>,
) -> String {
    let mut out = vec![GENERATED.to_string()];
    if !guides.is_empty() {
        out.extend(["".into(), "# Guides".into(), "".into()]);
        out.extend(
            guides.iter().map(|(name, guide)| {
                format!("* [{}]({name}) - {}", guide.title, guide.description)
            }),
        );
    }
    let open: Vec<(&String, &(Item, Sections))> = items
        .iter()
        .filter(|(_, (item, _))| item.status == Status::Stable)
        .collect();
    let mut tags: Vec<&str> = open
        .iter()
        .map(|(_, (item, _))| item.tag.as_str())
        .collect();
    tags.sort();
    tags.dedup();
    for tag in tags {
        out.extend(["".into(), format!("# {tag}"), "".into()]);
        let mut in_tag: Vec<_> = open
            .iter()
            .filter(|(_, (item, _))| item.tag == tag)
            .collect();
        in_tag.sort_by_key(|(name, (item, _))| (item.filed, *name));
        out.extend(
            in_tag
                .into_iter()
                .map(|(name, (item, sections))| open_line(name, item, sections)),
        );
    }
    let closed: Vec<_> = items
        .iter()
        .filter(|(_, (item, _))| item.status == Status::Deprecated)
        .collect();
    if !closed.is_empty() {
        out.extend(["".into(), "# Closed".into(), "".into()]);
        for (name, (item, sections)) in closed {
            out.push(format!(
                "* [{}]({name}) - {} | Resolution: {}",
                item.title,
                item.description,
                first_sentence(&sections[CLOSED_SECTION])
            ));
        }
    }
    out.join("\n") + "\n"
}

fn open_line(name: &str, item: &Item, sections: &Sections) -> String {
    // Only the date, in the time zone of the measurement: the time of day would not change what a reader does.
    // Whether stale_after has passed is not shown: the index would then depend on today's date, and the check that
    // compares it with the generated text would pass on some days and fail on others
    let measured = item.last_verified().at.date_naive();
    let deadline = match item.deadline_kind {
        DeadlineKind::Until => format!("Deadline: {}", item.deadline),
        DeadlineKind::NoDeadline => "No deadline.".to_string(),
    };
    let stale = item
        .stale_after
        .map(|at| format!(" | Re-measure after {}.", at.date_naive()))
        .unwrap_or_default();
    format!(
        "* [{}]({name}) - {} | State ({measured}): {} | {deadline}{stale}",
        item.title,
        item.description,
        first_sentence(&sections["State"])
    )
}

/// The index of `specs/` or `done/`. A spec in progress shows its status; a closed one its resolution.
pub fn render_specs(folder: &str, specs: &BTreeMap<String, (Spec, Sections)>) -> String {
    let heading = if folder == "specs" {
        "# Specs"
    } else {
        "# Closed specs"
    };
    let mut out = vec![GENERATED.to_string(), "".into(), heading.into(), "".into()];
    for (name, (spec, sections)) in specs {
        let after = if spec.status == Status::Deprecated {
            format!("Resolution: {}", first_sentence(&sections[CLOSED_SECTION]))
        } else {
            format!("Status: {}.", spec.status.name())
        };
        out.push(format!(
            "* [{}]({name}) - {} | {after}",
            spec.title, spec.description
        ));
    }
    out.join("\n") + "\n"
}

/// The bundle-root index. Only this index file may carry frontmatter (OKF 0.2, section 12).
pub fn render_root() -> String {
    let mut out: Vec<String> = [
        "---",
        "okf_version: \"0.2\"",
        "---",
        "",
        GENERATED,
        "",
        "# Records",
        "",
    ]
    .map(String::from)
    .into();
    out.extend(
        ROOT_ENTRIES
            .iter()
            .map(|(title, target, description)| format!("* [{title}]({target}) - {description}")),
    );
    out.join("\n") + "\n"
}

/// The open items past `stale_after`. As in OKF, an item is stale when `now >= stale_after`.
pub fn stale(items: &BTreeMap<String, (Item, Sections)>, now: Time) -> Vec<String> {
    items
        .iter()
        .filter(|(_, (item, _))| {
            item.status == Status::Stable && item.stale_after.is_some_and(|at| now >= at)
        })
        .map(|(name, _)| name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use super::*;

    const GOOD: &str = "---
type: Backlog Item
title: Some problem
description: Something is wrong.
tags: [operations]
status: stable
filed: 2026-09-27
verified: {by: human:someone, at: 2026-09-28T08:00:00+09:00}
deadline_kind: until
deadline: until the next deploy
---

# Trigger

The next deploy

# State

Not yet. Measured by hand.

# Details

[somewhere](/log.md)
";

    fn parsed(docs: &[(&str, String)]) -> Backlog {
        let docs: Docs = docs
            .iter()
            .map(|(name, text)| (name.to_string(), text.clone()))
            .collect();
        let parsed = backlog(&docs);
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        parsed
    }

    #[test]
    fn first_sentence_ends_at_a_full_stop_in_any_language() {
        let cases = [
            ("Not yet. Measured by hand.", "Not yet."),
            (
                "発火済み・未着手。手元の Rust は古い。",
                "発火済み・未着手。",
            ),
            ("**Bold first.** Then more.", "**Bold first.**"),
            ("Version 1.75 is old. Update it.", "Version 1.75 is old."),
            ("Ends at the end.", "Ends at the end."),
            ("No full stop", "No full stop"),
            ("  Padded.  \nsecond line.", "Padded."),
            ("**unclosed bold. Then", "**unclosed bold."),
            ("", ""),
        ];
        for (text, expected) in cases {
            assert_eq!(first_sentence(text), expected, "{text:?}");
        }
    }

    #[test]
    fn an_open_item_shows_its_state_and_deadline() {
        // Measured at 08:00 in +09:00, which is the day before in UTC: the date is the one where it was measured
        let index = render_backlog(&parsed(&[("good.md", GOOD.into())]).items, &BTreeMap::new());
        let line = "* [Some problem](good.md) - Something is wrong. | State (2026-09-28): Not yet. | Deadline: until \
                    the next deploy";
        assert_eq!(index, format!("{GENERATED}\n\n# operations\n\n{line}\n"));
    }

    #[test]
    fn the_stale_date_is_listed_but_not_judged() {
        // The index shows the stale_after date; whether it has passed is answered by `stale`, given the current time
        // (OKF: stale when now >= stale_after)
        let fresh = GOOD.replace(
            "deadline_kind:",
            "stale_after: 2027-03-31T00:00:00+09:00\ndeadline_kind:",
        );
        let parsed = parsed(&[("fresh.md", fresh), ("plain.md", GOOD.into())]);
        assert!(
            render_backlog(&parsed.items, &BTreeMap::new())
                .contains("| Re-measure after 2027-03-31.")
        );
        let cutoff = FixedOffset::east_opt(9 * 3600)
            .unwrap()
            .with_ymd_and_hms(2027, 3, 31, 0, 0, 0)
            .unwrap();
        assert_eq!(
            stale(&parsed.items, cutoff - chrono::Duration::seconds(1)),
            Vec::<String>::new()
        );
        assert_eq!(stale(&parsed.items, cutoff), vec!["fresh.md"]);
    }

    #[test]
    fn a_closed_item_leaves_the_open_list() {
        let closed =
            GOOD.replace("status: stable", "status: deprecated") + "\n# Resolution\n\nFixed.\n";
        let index = render_backlog(&parsed(&[("closed.md", closed)]).items, &BTreeMap::new());
        assert!(
            index.contains("# Closed") && index.contains("Fixed."),
            "{index}"
        );
        assert!(
            !index.contains("# operations"),
            "a closed item is still listed under its area"
        );
    }

    #[test]
    fn items_are_grouped_by_area_and_ordered_by_filing_date() {
        let later = GOOD.replace("filed: 2026-09-27", "filed: 2026-09-28");
        let other = GOOD.replace("tags: [operations]", "tags: [billing]");
        let parsed = parsed(&[("a.md", later), ("b.md", GOOD.into()), ("c.md", other)]);
        let index = render_backlog(&parsed.items, &BTreeMap::new());
        let order: Vec<&str> = index
            .lines()
            .filter(|line| line.starts_with('#') || line.starts_with('*'))
            .map(|line| line.split(')').next().unwrap())
            .collect();
        assert_eq!(
            order,
            [
                "# billing",
                "* [Some problem](c.md",
                "# operations",
                "* [Some problem](b.md",
                "* [Some problem](a.md"
            ]
        );
    }
}
