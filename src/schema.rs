//! The frontmatter of each record type: backlog items, guides and specs.
//!
//! OKF tells **readers** not to reject unknown fields. This is the **writer's** check, so unknown fields fail: a
//! misspelled field would otherwise be silently dropped. Every field OKF defines passes, so a document another OKF
//! tool wrote correctly does not fail.

use std::fmt::Display;
use std::sync::LazyLock;

use chrono::{DateTime, FixedOffset, NaiveDate};
use regex::Regex;
use yaml_rust2::Yaml;
use yaml_rust2::yaml::Hash;

use crate::frontmatter::{Sections, split};

/// A datetime with a time zone, as OKF writes every timestamp.
pub type Time = DateTime<FixedOffset>;

/// Body headings every backlog item needs. A closed item also needs `CLOSED_SECTION`
pub const SECTIONS: [&str; 3] = ["Trigger", "State", "Details"];
pub const CLOSED_SECTION: &str = "Resolution";

/// Directory -> the statuses a spec in it may have. The directory answers only "current or finished"
pub const SPEC_FOLDERS: [(&str, &[Status]); 2] = [
    ("specs", &[Status::Draft, Status::Stable]),
    ("done", &[Status::Deprecated]),
];

// OKF actors (section 7): `<producer>/<version>` for an agent, `human:<id>` for a person, `process:<id>`. OKF does not
// limit the characters of `<id>` (its own samples use `human:jsmith@acme`), so only whitespace is excluded
static ACTOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:human:\S+|process:\S+|[^\s:/]+/\S+)$").unwrap());
// A deadline that is only a date or a datetime. Deadlines are events, and the reason for having none is not a date
// either. Only the notation of the date is read, not the words around it, so an event in any language passes: the
// language of the content is left to the project
static DATE_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^\s*(?:
            \d{4}-\d{1,2}(?:-\d{1,2})?            # 2026-10-31, 2026-10
          | \d{4}/\d{1,2}(?:/\d{1,2})?            # 2026/10/31, 2026/10
          | \d{4}\.\d{1,2}\.\d{1,2}                # 2026.10.31 (2026.10 alone reads as a version)
          | \d{1,2}[-/.]\d{1,2}[-/.]\d{4}          # 31.10.2026, 10/31/2026
          | \d{4}\s*年\s*\d{1,2}\s*月(?:\s*\d{1,2}\s*日)?  # 2026年10月31日, 2026年10月
        )(?:[T\ ][\d:.]+(?:Z|[+-]\d{2}:?\d{2})?)?\s*$",
    )
    .unwrap()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Draft,
    Stable,
    Deprecated,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Stable => "stable",
            Status::Deprecated => "deprecated",
        }
    }
}

/// Who and when: one entry of OKF's `verified`, or `generated`.
#[derive(Debug, Clone, PartialEq)]
pub struct Stamp {
    pub by: String,
    pub at: Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineKind {
    /// Dropped if the trigger has not happened by the event in `deadline`
    Until,
    /// No deadline, with the reason in `deadline`
    NoDeadline,
}

/// The frontmatter of one backlog item.
#[derive(Debug, Clone)]
pub struct Item {
    pub title: String,
    /// What the problem is, in one sentence
    pub description: String,
    /// The area. The index groups items by it
    pub tag: String,
    /// `Stable` = open / `Deprecated` = closed. A closed item stays, so references to it keep working
    pub status: Status,
    pub filed: NaiveDate,
    /// Never empty. As in OKF, written as one mapping or a list of them
    pub verified: Vec<Stamp>,
    pub deadline_kind: DeadlineKind,
    pub deadline: String,
    /// When the recorded state goes out of date with time alone, the time after which it should be measured again.
    /// Unlike the deadline, it does not end the item, and passing it does not fail the check
    pub stale_after: Option<Time>,
}

impl Item {
    /// The last measurement: the newest `at`. On a tie, the one written first.
    pub fn last_verified(&self) -> &Stamp {
        // `max_by_key` keeps the last of equal keys, so search from the end to keep the first
        self.verified
            .iter()
            .rev()
            .max_by_key(|stamp| stamp.at)
            .unwrap()
    }
}

/// A document that is not an item, such as the backlog rules.
#[derive(Debug, Clone)]
pub struct Guide {
    pub title: String,
    pub description: String,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub title: String,
    pub description: String,
    pub status: Status,
}

/// A document in `docs/backlog/`.
#[derive(Debug, Clone)]
pub enum BacklogDoc {
    Item(Item, Sections),
    Guide(Guide),
}

/// A document in `docs/backlog/`, or why it breaks the backlog format.
pub fn backlog_doc(text: &str) -> Result<BacklogDoc, String> {
    let (meta, sections) = split(text)?;
    let mut fields = Fields::new(&meta);
    if fields.peek("type") == Some(&Yaml::String("Guide".into())) {
        let guide = guide(&mut fields);
        return fields.finish(guide).map(BacklogDoc::Guide);
    }
    let item = item(&mut fields);
    let item = fields.finish(item)?;
    let required = SECTIONS
        .iter()
        .copied()
        .chain((item.status == Status::Deprecated).then_some(CLOSED_SECTION));
    let empty: Vec<&str> = required
        .filter(|s| sections.get(*s).is_none_or(|text| text.is_empty()))
        .collect();
    if !empty.is_empty() {
        return Err(format!("body headings missing or empty: {empty:?}"));
    }
    Ok(BacklogDoc::Item(item, sections))
}

/// A spec in `docs/<folder>/`, or why it breaks the format or does not belong in that folder.
pub fn spec(folder: &str, text: &str) -> Result<(Spec, Sections), String> {
    let (meta, sections) = split(text)?;
    let mut fields = Fields::new(&meta);
    fields.required("type", one_of(&["Spec"]));
    let title = fields.required("title", non_empty_text);
    let description = fields.required("description", non_empty_text);
    let status = fields.required(
        "status",
        status(&[Status::Draft, Status::Stable, Status::Deprecated]),
    );
    fields.optional("tags", text_list);
    fields.optional("verified", stamps);
    fields.optional("stale_after", time);
    okf_optional(&mut fields);
    let spec = fields.finish(title.zip(description).zip(status).map(
        |((title, description), status)| Spec {
            title,
            description,
            status,
        },
    ))?;
    let allowed = SPEC_FOLDERS
        .iter()
        .find(|(name, _)| *name == folder)
        .map_or(&[][..], |(_, statuses)| *statuses);
    if !allowed.contains(&spec.status) {
        let names: Vec<&str> = allowed.iter().map(|s| s.name()).collect();
        return Err(format!(
            "status {} does not belong in {folder}/ ({})",
            spec.status.name(),
            names.join(", ")
        ));
    }
    if spec.status == Status::Deprecated
        && sections
            .get(CLOSED_SECTION)
            .is_none_or(|text| text.is_empty())
    {
        return Err(format!(
            "a closed spec needs a non-empty # {CLOSED_SECTION}"
        ));
    }
    Ok((spec, sections))
}

fn item(fields: &mut Fields) -> Option<Item> {
    fields.required("type", one_of(&["Backlog Item"]));
    let title = fields.required("title", non_empty_text);
    let description = fields.required("description", non_empty_text);
    let tag = fields.required("tags", one_tag);
    let status = fields.required("status", status(&[Status::Stable, Status::Deprecated]));
    let filed = fields.required("filed", date);
    let verified = fields.required("verified", stamps);
    let deadline_kind = fields.required("deadline_kind", deadline_kind);
    let deadline = fields.required("deadline", non_empty_text);
    let stale_after = fields.optional("stale_after", time);
    okf_optional(fields);
    let item = Item {
        title: title?,
        description: description?,
        tag: tag?,
        status: status?,
        filed: filed?,
        verified: verified?,
        deadline_kind: deadline_kind?,
        deadline: deadline?,
        stale_after: stale_after?,
    };
    if DATE_ONLY.is_match(&item.deadline) {
        fields.wrong(
            "deadline",
            format!("is an event or a reason, not a date: {}", item.deadline),
        );
    }
    if item.verified.is_empty() {
        fields.wrong("verified", "is an empty list");
        return None;
    }
    if let Some(stale_after) = item.stale_after {
        let last = item.last_verified().at;
        if stale_after <= last {
            fields.wrong(
                "stale_after",
                format!("({stale_after}) is not later than the last verified time ({last})"),
            );
        }
    }
    Some(item)
}

fn guide(fields: &mut Fields) -> Option<Guide> {
    fields.required("type", one_of(&["Guide"]));
    let title = fields.required("title", non_empty_text);
    let description = fields.required("description", non_empty_text);
    let status = fields
        .optional(
            "status",
            status(&[Status::Draft, Status::Stable, Status::Deprecated]),
        )
        .map(|status| status.unwrap_or(Status::Stable));
    fields.optional("tags", text_list);
    fields.optional("verified", stamps);
    fields.optional("stale_after", time);
    okf_optional(fields);
    Some(Guide {
        title: title?,
        description: description?,
        status: status?,
    })
}

/// Fields OKF defines that have no use here yet. They pass, so a document another OKF tool wrote does not fail.
fn okf_optional(fields: &mut Fields) {
    fields.optional("generated", stamp);
    fields.optional("sources", any_list);
    fields.optional("resource", text);
}

/// Reads the fields of one mapping, and remembers what was wrong and which keys were read. A key that no schema reads
/// is an unknown field.
struct Fields<'a> {
    map: &'a Hash,
    read: Vec<&'static str>,
    errors: Vec<String>,
}

/// The result of reading one field: `None` once the error is recorded.
type Read<T> = Option<T>;

impl<'a> Fields<'a> {
    fn new(map: &'a Hash) -> Self {
        Fields {
            map,
            read: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The value without reading it. A null counts as absent, as `X | None = None` does.
    fn peek(&self, key: &str) -> Option<&'a Yaml> {
        match self.map.get(&Yaml::String(key.into())) {
            None | Some(Yaml::Null) => None,
            Some(value) => Some(value),
        }
    }

    fn wrong(&mut self, key: &str, why: impl Display) {
        self.errors.push(format!("{key}: {why}"));
    }

    fn convert<T>(
        &mut self,
        key: &str,
        value: &Yaml,
        convert: impl Fn(&Yaml) -> Result<T, String>,
    ) -> Read<T> {
        convert(value).map_err(|why| self.wrong(key, why)).ok()
    }

    fn required<T>(
        &mut self,
        key: &'static str,
        convert: impl Fn(&Yaml) -> Result<T, String>,
    ) -> Read<T> {
        self.read.push(key);
        match self.peek(key) {
            Some(value) => self.convert(key, value, convert),
            None => {
                self.wrong(key, "missing");
                None
            }
        }
    }

    /// `Some(None)` when absent, `None` when present and wrong.
    fn optional<T>(
        &mut self,
        key: &'static str,
        convert: impl Fn(&Yaml) -> Result<T, String>,
    ) -> Read<Option<T>> {
        self.read.push(key);
        match self.peek(key) {
            Some(value) => self.convert(key, value, convert).map(Some),
            None => Some(None),
        }
    }

    /// The value when every field passed and no key was unknown, or every error.
    fn finish<T>(mut self, value: Option<T>) -> Result<T, String> {
        for key in self.map.keys() {
            match key {
                Yaml::String(name) if self.read.contains(&name.as_str()) => {}
                Yaml::String(name) => self.errors.push(format!("unknown field: {name}")),
                other => self.errors.push(format!("unknown field: {other:?}")),
            }
        }
        match value {
            Some(value) if self.errors.is_empty() => Ok(value),
            _ => Err(self.errors.join("; ")),
        }
    }
}

// --- converters: one YAML value to one typed value -----------------------------

/// A string. Strict as in the Python checks: a number or a boolean is not turned into text.
fn text(value: &Yaml) -> Result<String, String> {
    match value {
        Yaml::String(s) => Ok(s.clone()),
        other => Err(format!("not a string: {other:?}")),
    }
}

/// A string with something in it: spaces alone are as empty as nothing.
fn non_empty_text(value: &Yaml) -> Result<String, String> {
    text(value).and_then(|s| {
        if s.trim().is_empty() {
            Err("empty".into())
        } else {
            Ok(s)
        }
    })
}

fn one_of(allowed: &'static [&'static str]) -> impl Fn(&Yaml) -> Result<String, String> {
    move |value| {
        text(value).and_then(|s| {
            if allowed.contains(&s.as_str()) {
                Ok(s)
            } else {
                Err(format!("{s:?} is not one of {allowed:?}"))
            }
        })
    }
}

fn status(allowed: &'static [Status]) -> impl Fn(&Yaml) -> Result<Status, String> {
    move |value| {
        let s = text(value)?;
        allowed
            .iter()
            .copied()
            .find(|status| status.name() == s)
            .ok_or_else(|| {
                let names: Vec<&str> = allowed.iter().map(|status| status.name()).collect();
                format!("{s:?} is not one of {names:?}")
            })
    }
}

fn deadline_kind(value: &Yaml) -> Result<DeadlineKind, String> {
    match text(value)?.as_str() {
        "until" => Ok(DeadlineKind::Until),
        "none" => Ok(DeadlineKind::NoDeadline),
        other => Err(format!("{other:?} is not one of [\"until\", \"none\"]")),
    }
}

fn date(value: &Yaml) -> Result<NaiveDate, String> {
    let s = text(value)?;
    NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| format!("not a date (YYYY-MM-DD): {s}"))
}

/// A datetime with a time zone. A date alone fails: filling in 00:00 would invent a time nobody measured.
fn time(value: &Yaml) -> Result<Time, String> {
    let s = text(value)?;
    DateTime::parse_from_rfc3339(&s)
        .map_err(|_| format!("not a datetime with a time zone (ISO 8601): {s}"))
}

fn stamp(value: &Yaml) -> Result<Stamp, String> {
    let Yaml::Hash(map) = value else {
        return Err(format!("not a {{by, at}} mapping: {value:?}"));
    };
    let mut fields = Fields::new(map);
    let by = fields.required("by", |value| {
        text(value).and_then(|s| {
            if ACTOR.is_match(&s) {
                Ok(s)
            } else {
                Err(format!(
                    "not an actor (human:<id>, process:<id> or <producer>/<version>): {s}"
                ))
            }
        })
    });
    let at = fields.required("at", time);
    fields.finish(by.zip(at).map(|(by, at)| Stamp { by, at }))
}

/// One `{by, at}` mapping or a list of them.
fn stamps(value: &Yaml) -> Result<Vec<Stamp>, String> {
    match value {
        Yaml::Array(list) => list.iter().map(stamp).collect(),
        other => stamp(other).map(|one| vec![one]),
    }
}

/// A list of tags. An empty tag names no area.
fn text_list(value: &Yaml) -> Result<Vec<String>, String> {
    match value {
        Yaml::Array(list) => list.iter().map(non_empty_text).collect(),
        other => Err(format!("not a list: {other:?}")),
    }
}

/// Exactly one tag: the area the index groups the item by.
fn one_tag(value: &Yaml) -> Result<String, String> {
    let mut tags = text_list(value)?;
    match tags.len() {
        1 => Ok(tags.remove(0)),
        n => Err(format!("{n} tags; an item has exactly one area")),
    }
}

fn any_list(value: &Yaml) -> Result<(), String> {
    match value {
        Yaml::Array(_) => Ok(()),
        other => Err(format!("not a list: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid item. Each bad input below changes exactly one thing in it
    const GOOD: &str = "---
type: Backlog Item
title: Some problem
description: Something is wrong.
tags: [operations]
status: stable
filed: 2026-09-27
verified: {by: human:someone, at: 2026-09-28T10:00:00+09:00}
deadline_kind: until
deadline: until the next deploy
---

# Trigger

The next deploy

# State

Not yet.

# Details

[somewhere](/log.md)
";
    const AT: &str = "at: 2026-09-28T10:00:00+09:00";

    fn good(from: &str, to: &str) -> String {
        assert!(
            GOOD.contains(from),
            "{from:?} is not in GOOD, so the case would change nothing"
        );
        GOOD.replacen(from, to, 1)
    }

    fn passing(cases: &[(&str, String)]) -> Vec<String> {
        cases
            .iter()
            .filter_map(|(name, text)| backlog_doc(text).ok().map(|_| name.to_string()))
            .collect()
    }

    fn failing(cases: &[(&str, String)]) -> Vec<(String, String)> {
        cases
            .iter()
            .filter_map(|(name, text)| backlog_doc(text).err().map(|why| (name.to_string(), why)))
            .collect()
    }

    #[test]
    fn the_good_input_passes() {
        // If the valid item did not pass, a failure below would not show that the one change was caught
        assert!(backlog_doc(GOOD).is_ok(), "{:?}", backlog_doc(GOOD).err());
    }

    #[test]
    fn what_okf_allows_passes() {
        let two = "verified:\n  - {by: human:someone, at: 2026-09-28T10:00:00+09:00}\
                   \n  - {by: process:nightly, at: 2026-09-29T02:00:00Z}";
        let one = "verified: {by: human:someone, at: 2026-09-28T10:00:00+09:00}";
        let okf = [
            ("verified as a list", good(one, two)),
            (
                "optional OKF fields",
                good(
                    "status: stable",
                    "status: stable\ngenerated: {by: agent/v1, at: 2026-09-27T09:00:00Z}\
                     \nresource: https://example.com/x\nsources: [{resource: https://example.com/doc}]",
                ),
            ),
            // The content may be in any language
            (
                "a deadline in Japanese",
                good(
                    "deadline: until the next deploy",
                    "deadline: 次のデプロイまで",
                ),
            ),
            // An event may carry a date; only a date alone is not an event
            (
                "an event with a date in it",
                good(
                    "deadline: until the next deploy",
                    "deadline: until the release planned for 2026/10/31",
                ),
            ),
            (
                "an event with a Japanese date in it",
                good(
                    "deadline: until the next deploy",
                    "deadline: 2026年10月31日のリリースまで",
                ),
            ),
            // Quoting does not change a value (YAML 1.2). PyYAML read a quoted datetime as a string, and it failed
            (
                "a quoted datetime",
                good(AT, "at: '2026-09-28T10:00:00+09:00'"),
            ),
            (
                "a quoted date",
                good("filed: 2026-09-27", "filed: '2026-09-27'"),
            ),
            // OKF does not limit the characters of an actor's id; its own samples use this one
            (
                "an actor with an at sign",
                good("human:someone", "human:jsmith@acme"),
            ),
            (
                "no deadline, with the reason",
                good("deadline_kind: until", "deadline_kind: none"),
            ),
        ];
        let failed = failing(&okf);
        assert!(failed.is_empty(), "{failed:?}");
        // The newest measurement is the last one (used for the index date and for stale_after)
        let Ok(BacklogDoc::Item(item, _)) = backlog_doc(&okf[0].1) else {
            panic!()
        };
        assert_eq!(item.last_verified().by, "process:nightly");
    }

    #[test]
    fn a_broken_document_is_caught() {
        let bad = [
            ("no trigger", good("# Trigger\n\nThe next deploy\n\n", "")),
            ("empty state", good("Not yet.\n", "")),
            // A reader of the rendered page sees neither a comment nor what a code block holds
            (
                "a state that is only a comment",
                good("Not yet.\n", "<!-- Not yet. -->\n"),
            ),
            (
                "headings only inside an unclosed code block",
                good("The next deploy\n", "The next deploy\n\n```\n"),
            ),
            ("no verified time", good(&format!(", {AT}"), "")),
            // OKF asks for a datetime. With a date only, someone would have to invent the time
            ("verified date only", good(AT, "at: 2026-09-28")),
            (
                "verified without a time zone",
                good(AT, "at: 2026-09-28T10:00:00"),
            ),
            (
                "empty verified list",
                good(
                    &format!("verified: {{by: human:someone, {AT}}}"),
                    "verified: []",
                ),
            ),
            ("actor in the wrong form", good("human:someone", "someone")),
            (
                "actor with a space",
                good("human:someone", "'human:some one'"),
            ),
            (
                "deadline is a date",
                good("deadline: until the next deploy", "deadline: 2026-12-31"),
            ),
            (
                "deadline is a quoted date",
                good(
                    "deadline: until the next deploy",
                    "deadline: \"2026-12-31\"",
                ),
            ),
            (
                "deadline is a datetime",
                good(
                    "deadline: until the next deploy",
                    "deadline: \"2026-12-31T00:00:00+09:00\"",
                ),
            ),
            // A date in another notation is still only a date
            (
                "deadline is a date with slashes",
                good("deadline: until the next deploy", "deadline: 2026/10/31"),
            ),
            (
                "deadline is a date with dots, day first",
                good("deadline: until the next deploy", "deadline: 31.10.2026"),
            ),
            (
                "deadline is a date, month first",
                good("deadline: until the next deploy", "deadline: 10/31/2026"),
            ),
            (
                "deadline is a month",
                good("deadline: until the next deploy", "deadline: 2026-10"),
            ),
            (
                "deadline is a date in Japanese",
                good(
                    "deadline: until the next deploy",
                    "deadline: 2026年10月31日",
                ),
            ),
            (
                "deadline is a month in Japanese",
                good("deadline: until the next deploy", "deadline: 2026年10月"),
            ),
            (
                "deadline is a date and a time with slashes",
                good(
                    "deadline: until the next deploy",
                    "deadline: \"2026/10/31 18:00\"",
                ),
            ),
            // The reason for having no deadline is not a date either
            (
                "no deadline, with a date for the reason",
                good(
                    "deadline_kind: until\ndeadline: until the next deploy",
                    "deadline_kind: none\ndeadline: 2026-12-31",
                ),
            ),
            ("no deadline kind", good("deadline_kind: until\n", "")),
            (
                "unknown field",
                good("status: stable", "status: stable\nstatu: stable"),
            ),
            (
                "closed without a resolution",
                good("status: stable", "status: deprecated"),
            ),
            (
                "no frontmatter",
                GOOD.splitn(3, "---\n").nth(2).unwrap().to_string(),
            ),
            (
                "unquoted colon",
                good(
                    "description: Something is wrong.",
                    "description: how to measure: count",
                ),
            ),
            (
                "a number for a title",
                good("title: Some problem", "title: 123"),
            ),
            (
                "two tags",
                good("tags: [operations]", "tags: [operations, other]"),
            ),
            // Spaces alone are as empty as nothing
            ("a blank title", good("title: Some problem", "title: \" \"")),
            (
                "a blank deadline",
                good("deadline: until the next deploy", "deadline: \"  \""),
            ),
            ("an empty tag", good("tags: [operations]", "tags: [\"\"]")),
            ("a blank tag", good("tags: [operations]", "tags: [\" \"]")),
            (
                "filed is not a date",
                good("filed: 2026-09-27", "filed: 2026-13-01"),
            ),
            (
                "stale_after equal to the verified time",
                good(
                    "deadline_kind:",
                    "stale_after: 2026-09-28T10:00:00+09:00\ndeadline_kind:",
                ),
            ),
            (
                "stale_after date only",
                good("deadline_kind:", "stale_after: 2027-03-31\ndeadline_kind:"),
            ),
            (
                "stale_after not a date",
                good(
                    "deadline_kind:",
                    "stale_after: in six months\ndeadline_kind:",
                ),
            ),
            (
                "a spec in the backlog",
                good("type: Backlog Item", "type: Spec"),
            ),
        ];
        let passed = passing(&bad);
        assert!(passed.is_empty(), "passed: {passed:?}");
    }

    #[test]
    fn a_guide_passes_with_its_own_fields() {
        let rules = "---\ntype: Guide\ntitle: Backlog rules\ndescription: What goes here.\n---\n\n# What goes here\n";
        let Ok(BacklogDoc::Guide(guide)) = backlog_doc(rules) else {
            panic!("{:?}", backlog_doc(rules))
        };
        assert_eq!(guide.status, Status::Stable);
        for bad in [
            rules.replace("description: What goes here.\n", ""),
            rules.replace("type: Guide", "type: Guide\ndeadline: never"),
        ] {
            assert!(backlog_doc(&bad).is_err(), "{bad}");
        }
    }

    const SPEC: &str = "---
type: Spec
title: Something
description: One sentence.
status: stable
---

# Goals

Something.
";

    #[test]
    fn a_good_spec_passes() {
        assert!(
            spec("specs", SPEC).is_ok(),
            "{:?}",
            spec("specs", SPEC).err()
        );
        let closed =
            SPEC.replace("status: stable", "status: deprecated") + "\n# Resolution\n\nDone.\n";
        assert!(
            spec("done", &closed).is_ok(),
            "{:?}",
            spec("done", &closed).err()
        );
    }

    #[test]
    fn a_broken_spec_is_caught() {
        let closed = SPEC.replace("status: stable", "status: deprecated");
        let bad = [
            (
                "specs",
                "closed spec still in specs/",
                closed.clone() + "\n# Resolution\n\nDone.\n",
            ),
            (
                "specs",
                "no description",
                SPEC.replace("description: One sentence.\n", ""),
            ),
            (
                "specs",
                "a blank description",
                SPEC.replace("description: One sentence.", "description: \" \""),
            ),
            (
                "specs",
                "an empty tag",
                SPEC.replace("status: stable", "status: stable\ntags: [\"\"]"),
            ),
            (
                "specs",
                "unknown field",
                SPEC.replace("status: stable", "status: stable\nstatu: stable"),
            ),
            (
                "specs",
                "date-only verified",
                SPEC.replace(
                    "status: stable",
                    "status: stable\nverified: {by: human:a, at: 2026-10-01}",
                ),
            ),
            (
                "specs",
                "no frontmatter",
                "# Goals\n\nSomething.\n".to_string(),
            ),
            ("done", "open spec in done/", SPEC.to_string()),
            ("done", "closed without a resolution", closed),
        ];
        for (folder, name, text) in bad {
            assert!(spec(folder, &text).is_err(), "{folder}: {name} passed");
        }
    }
}
