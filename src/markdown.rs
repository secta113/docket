//! Markdown headings and links, read the way GitHub reads them. The records link to each other with standard markdown
//! links (OKF 0.2, section 6.1), and a link to a heading uses GitHub's anchor for that heading.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;
use regex::Regex;
use unicode_general_category::{GeneralCategory, get_general_category};

use crate::source::read_source;

// Not rendered, so no heading inside them gets an anchor: HTML comments and fenced code blocks
static HIDDEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?sm)<!--.*?-->|^```.*?^```").unwrap());
static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#{1,6}[ \t]+(.+?)[ \t]*$").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)\s]+)\)").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:").unwrap());
// A drive letter has the form of a one-letter URL scheme. No scheme has one letter
static DRIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z]:").unwrap());

/// The text without the parts that are not rendered.
pub fn visible(text: &str) -> String {
    HIDDEN.replace_all(text, "").into_owned()
}

/// Letters, numbers and marks: the Unicode categories GitHub keeps in an anchor.
fn is_letter_number_or_mark(c: char) -> bool {
    use GeneralCategory::*;
    matches!(
        get_general_category(c),
        UppercaseLetter
            | LowercaseLetter
            | TitlecaseLetter
            | ModifierLetter
            | OtherLetter
            | DecimalNumber
            | LetterNumber
            | OtherNumber
            | NonspacingMark
            | SpacingMark
            | EnclosingMark
    )
}

/// GitHub's anchor for a heading: lowercase, keep letters, digits, marks, `-`, `_` and spaces, then spaces to `-`.
///
/// Characters outside those classes (punctuation and symbols, full-width ones included) are dropped. Each space
/// becomes one hyphen, so two spaces give two hyphens.
///
/// The categories come from the Unicode version of `unicode-general-category`, newer than the 15.0 of Python 3.12: the
/// 5057 code points assigned after 15.0 are kept here and dropped by the Python checks (measured 2026-10-02). Every
/// other code point gives the same anchor in both.
pub fn slug(heading: &str) -> String {
    heading
        .to_lowercase()
        .chars()
        .filter(|&c| matches!(c, ' ' | '-' | '_') || is_letter_number_or_mark(c))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Every heading anchor in a markdown text. A repeated heading gets `-1`, `-2`, ... as on GitHub.
pub fn anchors(text: &str) -> HashSet<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut found = HashSet::new();
    for caps in HEADING.captures_iter(&visible(text)) {
        let base = slug(&caps[1]);
        let count = seen.entry(base.clone()).or_insert(0);
        found.insert(if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        });
        *count += 1;
    }
    found
}

/// `(text, target)` for every inline markdown link.
pub fn links(text: &str) -> Vec<(String, String)> {
    LINK.captures_iter(text)
        .map(|caps| (caps[1].to_string(), caps[2].to_string()))
        .collect()
}

/// Why the link does not resolve, or `None` when it does.
///
/// `here` is the directory of the document that holds the link, for relative targets. A target starting with `/` is
/// relative to `bundle_root`. A URL is not checked. A path with a drive letter (`C:/...`) fails: it names a file on one
/// machine, which no other checkout and no reader on GitHub can follow. A link to a `.py` file names a function or
/// class in its text.
pub fn broken(text: &str, target: &str, here: &Path, bundle_root: &Path) -> Option<String> {
    if DRIVE.is_match(target) {
        return Some(format!(
            "a path on one machine; link with / from the bundle root, or with a relative path: {target}"
        ));
    }
    if URL.is_match(target) {
        return None;
    }
    let decoded = percent_decode_str(target).decode_utf8_lossy();
    let (path, fragment) = decoded.split_once('#').unwrap_or((&decoded, ""));
    if path.is_empty() {
        return Some(format!("no file in the link: {target}"));
    }
    let full = match path.strip_prefix('/') {
        Some(rest) => bundle_root.join(rest.trim_start_matches('/')),
        None => here.join(path),
    };
    if !full.is_file() {
        return Some(format!("no such file: {target}"));
    }
    let source = match read_source(&full) {
        Ok(source) => source,
        Err(e) => return Some(format!("cannot read {target}: {e}")),
    };
    if path.ends_with(".py") {
        let name = text.trim().trim_matches('`');
        // The name has to be defined, not only mentioned: a call or a comment can keep a name after the definition
        // was renamed
        let defined = Regex::new(&format!(
            r"(?m)^\s*(?:async\s+)?(?:def|class)\s+{}\b",
            regex::escape(name)
        ))
        .unwrap();
        if !defined.is_match(&source) {
            return Some(format!("no def or class named {name} in {target}"));
        }
    } else if !fragment.is_empty() && !anchors(&source).contains(fragment) {
        return Some(format!("no heading with this anchor: {target}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// heading -> the anchor GitHub gave it. Each value was read from a page that GitHub rendered, so a change to
    /// `slug` that drifts from GitHub fails here instead of breaking links silently.
    const SEEN_ON_GITHUB: &[(&str, &str)] = &[
        // project-template README.md, rendered 2026-10-01
        ("project-template", "project-template"),
        ("始め方", "始め方"),
        ("雛形の改善を取り込む", "雛形の改善を取り込む"),
        (
            "中身（写した先の根になるもの）",
            "中身写した先の根になるもの",
        ),
        // project-template docs/log.md, rendered 2026-10-02: a slash, an ideographic comma and spaces
        (
            "配る中身を template/ に下ろし、copier で写す",
            "配る中身を-template-に下ろしcopier-で写す",
        ),
        (
            "雛形を作り、グローバルの既定を実体にした",
            "雛形を作りグローバルの既定を実体にした",
        ),
    ];

    #[test]
    fn anchors_match_github() {
        for (heading, anchor) in SEEN_ON_GITHUB {
            assert_eq!(slug(heading), *anchor, "{heading}");
        }
    }

    #[test]
    fn repeated_and_hidden_headings() {
        let text = "# Same\n\n## Same\n\n<!--\n## Hidden\n-->\n\n```\n## Code\n```\n";
        let expected: HashSet<String> = ["same", "same-1"].map(String::from).into();
        assert_eq!(anchors(text), expected);
    }

    #[test]
    fn links_are_found_in_order() {
        let text = "[a](/x.md#y)、[`b`](../c.py) and [d](https://example.com)";
        let pairs = [
            ("a", "/x.md#y"),
            ("`b`", "../c.py"),
            ("d", "https://example.com"),
        ];
        let expected: Vec<(String, String)> = pairs
            .iter()
            .map(|(t, l)| (t.to_string(), l.to_string()))
            .collect();
        assert_eq!(links(text), expected);
    }

    /// A bundle with a document, a log whose format guide is an HTML comment, and a Python file outside the bundle.
    /// Laid out as in a project: the links are written from `docs/backlog/`.
    fn tree() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let docs = root.path().join("docs");
        fs::create_dir_all(docs.join("backlog")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::write(
            docs.join("backlog/rules.md"),
            "# Rules\n\n## What goes here\n",
        )
        .unwrap();
        fs::write(docs.join("log.md"), "# Log\n\n<!--\n### Task name\n-->\n").unwrap();
        fs::write(
            root.path().join("tests/backlog_bundle.py"),
            "def render_index():\n    pass\n\n# gone() is only mentioned\n",
        )
        .unwrap();
        root
    }

    fn reasons(root: &Path, cases: &[(&str, &str)]) -> Vec<Option<String>> {
        let docs = root.join("docs");
        cases
            .iter()
            .map(|(text, target)| broken(text, target, &docs.join("backlog"), &docs))
            .collect()
    }

    #[test]
    fn a_resolving_link_passes() {
        let root = tree();
        let resolved = [
            (
                "heading from the bundle root",
                "/backlog/rules.md#what-goes-here",
            ),
            ("heading by a relative path", "rules.md#what-goes-here"),
            ("a file", "/log.md"),
            ("`render_index`", "../../tests/backlog_bundle.py"),
            ("a URL is not checked", "https://example.com/okf"),
        ];
        assert_eq!(reasons(root.path(), &resolved), vec![None; resolved.len()]);
    }

    #[test]
    fn a_dangling_link_is_caught() {
        let root = tree();
        let bad = [
            ("missing heading", "/backlog/rules.md#no-such-heading"),
            ("missing file", "/no_such_file.md"),
            (
                "`no_such_function_anywhere`",
                "../../tests/backlog_bundle.py",
            ),
            // Mentioned in a comment, not defined
            ("`gone`", "../../tests/backlog_bundle.py"),
            // The format guide at the top of the log is an HTML comment, so GitHub gives its headings no anchor
            ("heading inside a comment", "/log.md#task-name"),
            ("only a fragment", "#what-goes-here"),
            // A drive letter is not a URL scheme: the path names a file on one machine
            ("drive letter", "C:/no/such/file.md"),
            ("drive letter with backslashes", "c:\\no\\such\\file.md"),
        ];
        let found = reasons(root.path(), &bad);
        for ((text, target), why) in bad.iter().zip(&found) {
            assert!(why.is_some(), "{text} ({target}) passed");
        }
    }
}
