//! The marker check: no comment in the code holds `TODO`, `FIXME`, `XXX`, `HACK` or `NOTE`.
//!
//! - A `TODO` (and `FIXME`, `XXX`, `HACK`) is work left to do that no record holds: nothing lists it, and nothing
//!   asks when it is done. A `NOTE` is knowledge kept where nothing can filter or link it.
//! - The words count in upper case, as whole words, and only in comments: `Status.TODO` in a task board and
//!   `"XXX-XXXX"` in a phone format are not markers. Docstrings are strings, so they are not read.
//! - Every code file of the layout is read, `tests/` and the other paths that are not layers included, except the
//!   paths the project lists in `unchecked` (generated code is written by a tool the project does not edit).
//!
//! Python only: for `rust` and `typescript`, the check says that it did not run. The floor: at least one code file
//! is read, or the check fails instead of passing with nothing read.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use ruff_python_ast::PySourceType;
use ruff_python_ast::token::TokenKind;

use crate::layers::Declared;
use crate::source::read_source;
use crate::structure::{code_files, within};

/// The words that fail in a comment.
pub const MARKERS: [&str; 5] = ["TODO", "FIXME", "XXX", "HACK", "NOTE"];

static MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\b(?:{})\b", MARKERS.join("|"))).unwrap());

/// What the marker check found.
#[derive(Debug, Default)]
pub struct Markers {
    /// Every comment that holds a marker, as `path:line` and the line under it as a traceback shows it, and every
    /// file that could not be read
    pub found: Vec<String>,
    /// The markers found, in the order of [`MARKERS`], each once
    pub words: Vec<&'static str>,
    /// What was not checked, and why
    pub skipped: Option<String>,
}

impl Markers {
    /// The heading the findings are printed under: the markers found, and where the text goes instead. All of them
    /// when none was found (a file that could not be read, or no file at all).
    pub fn heading(&self) -> String {
        let words = if self.words.is_empty() {
            &MARKERS[..]
        } else {
            &self.words[..]
        };
        let listed = match words {
            [one] => one.to_string(),
            [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
            [] => unreachable!("MARKERS is not empty"),
        };
        format!(
            "no comment holds {listed} (work left to do goes in docs/backlog/, a decision and its reason in the spec \
             or the log entry of the change, how to read the code in a plain comment without the word)"
        )
    }
}

/// Every marker in a comment of the code of `declared`. An error is a directory that could not be walked.
pub fn problems(root: &Path, declared: &Declared) -> io::Result<Markers> {
    let Some(layout) = &declared.layout else {
        // A repository of records only has no code to read
        return Ok(Markers::default());
    };
    if declared.declaration.stack != "python" {
        return Ok(Markers {
            skipped: Some(format!(
                "comments are not checked for {}: Rotproof does not read the comments of a {} project yet",
                MARKERS.join(", "),
                declared.declaration.stack
            )),
            ..Markers::default()
        });
    }
    let unchecked: Vec<&str> = declared
        .declaration
        .unchecked
        .iter()
        .map(|path| path.trim_end_matches('/'))
        .collect();
    let mut found = Vec::new();
    let mut words = BTreeSet::new();
    let mut read = 0;
    for path in code_files(root, layout, &layout.scope)? {
        if unchecked.iter().any(|skip| within(&path, skip)) {
            continue;
        }
        let source = match read_source(&root.join(&path)) {
            Ok(source) => source,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                found.push(format!(
                    "{path}: cannot be read as UTF-8, so its comments are not checked"
                ));
                continue;
            }
            Err(e) => return Err(io::Error::new(e.kind(), format!("{path}: {e}"))),
        };
        read += 1;
        let lines: Vec<&str> = source.lines().collect();
        for (line, in_comment) in markers(&source) {
            found.push(format!("{path}:{line}\n{}", lines[line - 1].trim()));
            words.extend(in_comment);
        }
    }
    if read == 0 {
        found.push(format!(
            "no code file was read: the {} layout finds none in the tree",
            declared.declaration.stack
        ));
    }
    Ok(Markers {
        found,
        // In the order of MARKERS: a set of their positions
        words: words.into_iter().map(|i| MARKERS[i]).collect(),
        skipped: None,
    })
}

/// Every comment of a Python source that holds a marker: its line (from 1), and the markers in it as positions in
/// [`MARKERS`]. The lexer reads comments past a syntax error too; the direction check reports the error.
fn markers(source: &str) -> Vec<(usize, Vec<usize>)> {
    let parsed = ruff_python_parser::parse_unchecked_source(source, PySourceType::Python);
    let mut found = Vec::new();
    for token in parsed.tokens().iter() {
        let (kind, range) = token.as_tuple();
        if kind != TokenKind::Comment {
            continue;
        }
        let comment = &source[range.start().to_usize()..range.end().to_usize()];
        let words: Vec<usize> = MARKER
            .find_iter(comment)
            .map(|word| MARKERS.iter().position(|m| *m == word.as_str()).unwrap())
            .collect();
        if !words.is_empty() {
            found.push((line_of(source, range.start().to_usize()), words));
        }
    }
    found
}

/// The line (from 1) at a byte offset.
fn line_of(source: &str, at: usize) -> usize {
    source.as_bytes()[..at.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(source: &str) -> Vec<(usize, Vec<&'static str>)> {
        markers(source)
            .into_iter()
            .map(|(line, found)| (line, found.into_iter().map(|i| MARKERS[i]).collect()))
            .collect()
    }

    #[test]
    fn every_marker_fails_in_a_comment() {
        for marker in MARKERS {
            assert_eq!(
                words(&format!("x = 1\n# {marker}: later\n")),
                [(2, vec![marker])],
                "{marker}"
            );
        }
        // One comment, one finding, with every marker in it
        assert_eq!(
            words("x = 1  # TODO(me) and NOTE\n"),
            [(1, vec!["TODO", "NOTE"])]
        );
    }

    #[test]
    fn the_heading_names_only_the_markers_found() {
        let heading = |words: Vec<&'static str>| {
            let found = Markers {
                words,
                ..Markers::default()
            };
            let heading = found.heading();
            heading[..heading.find(" (").unwrap()].to_string()
        };
        assert_eq!(heading(vec!["NOTE"]), "no comment holds NOTE");
        assert_eq!(
            heading(vec!["TODO", "NOTE"]),
            "no comment holds TODO or NOTE"
        );
        assert_eq!(
            heading(vec!["TODO", "XXX", "NOTE"]),
            "no comment holds TODO, XXX or NOTE"
        );
        // Nothing found but a file that could not be read: every marker
        assert_eq!(
            heading(Vec::new()),
            "no comment holds TODO, FIXME, XXX, HACK or NOTE"
        );
    }

    #[test]
    fn a_marker_outside_a_comment_or_in_another_word_passes() {
        let source = "\
class Status:
    TODO = 1
    \"\"\"TODO in a docstring is a string.\"\"\"
phone = \"XXX-XXXX\"
note = Status.TODO  # todo in lower case, TODOS and NOTES, NOTE_X and XXXL are other words
";
        assert_eq!(words(source), []);
    }

    #[test]
    fn comments_after_a_syntax_error_are_read() {
        assert_eq!(words("def f(:\n    pass\n# FIXME\n"), [(3, vec!["FIXME"])]);
    }
}
