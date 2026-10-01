//! Markdown read the way GitHub reads it: the text it shows, headings and links. The records link to each other with
//! standard markdown links (OKF 0.2, section 6.1), and a link to a heading uses GitHub's anchor for that heading.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;
use regex::Regex;
use unicode_general_category::{GeneralCategory, get_general_category};

use crate::source::read_source;

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)\s]+)\)").unwrap());
// A URL scheme. RFC 3986 allows `.` in one, but no scheme in use has it, while a file name with a line number
// (`check.rs:104`) always does: read as a URL, that path would never be checked
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+-]*:").unwrap());
// A drive letter has the form of a one-letter URL scheme. No scheme has one letter
static DRIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z]:").unwrap());

/// The text without the parts GitHub does not render as text: HTML comments and fenced code blocks. Every check that
/// reads the structure of a document (its sections, headings and links) reads this, so a heading or a link that a
/// reader of the page cannot see counts for nothing.
///
/// Read as GFM reads them. A fence is a line of 3 or more backticks or tildes, indented by up to 3 spaces; a backtick
/// fence has no backtick after it on the line, or the line is inline code. It is closed by a line of the same
/// character, at least as long, indented by up to 3 spaces, and an unclosed fence runs to the end. A comment runs from
/// `<!--` to the next `-->`, across lines, and the text around it stays; inside a fence, `<!--` is code.
pub fn visible(text: &str) -> String {
    let mut out = String::new();
    let mut fence: Option<(char, usize)> = None;
    // Inside a comment: the text before it on its first line, which the text after its end joins
    let mut comment: Option<String> = None;
    for line in text.split_inclusive('\n') {
        if let Some((mark, length)) = fence {
            if fence_length(line, mark).is_some_and(|n| n >= length) && is_fence_end(line, mark) {
                fence = None;
            }
            continue;
        }
        let mut rest = line;
        let mut shown = match comment.take() {
            Some(before) => match rest.find("-->") {
                Some(end) => {
                    rest = &rest[end + 3..];
                    before
                }
                None => {
                    comment = Some(before);
                    continue;
                }
            },
            None => {
                if let Some(opened) = fence_opening(line) {
                    fence = Some(opened);
                    continue;
                }
                String::new()
            }
        };
        let mut open = false;
        while let Some(start) = rest.find("<!--") {
            shown.push_str(&rest[..start]);
            match rest[start + 4..].find("-->") {
                Some(end) => rest = &rest[start + 4 + end + 3..],
                None => {
                    open = true;
                    rest = "";
                    break;
                }
            }
        }
        shown.push_str(rest);
        if open {
            comment = Some(shown);
        } else {
            out.push_str(&shown);
        }
    }
    out
}

/// The run of `mark` that starts a line, after up to 3 spaces: its length, when it is long enough for a fence.
fn fence_length(line: &str, mark: char) -> Option<usize> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let length = line[indent..].chars().take_while(|&c| c == mark).count();
    (indent <= 3 && length >= 3).then_some(length)
}

/// The fence a line opens: its character and its length.
fn fence_opening(line: &str) -> Option<(char, usize)> {
    ['`', '~'].into_iter().find_map(|mark| {
        let length = fence_length(line, mark)?;
        // The marks are ASCII, so the run's length in characters is its length in bytes
        let info = &line.trim_start_matches(' ')[length..];
        (mark == '~' || !info.contains('`')).then_some((mark, length))
    })
}

/// Whether a line that starts with a run of `mark` has nothing after the run, as a closing fence has.
fn is_fence_end(line: &str, mark: char) -> bool {
    line.trim_start_matches(' ')
        .trim_start_matches(mark)
        .trim()
        .is_empty()
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

/// The level and the text of a heading, when the line is one, read as GFM reads it: up to 3 spaces, 1 to 6 `#`, then
/// a space or a tab before the text. A closing run of `#` after a space is not part of the text. Every check that looks
/// for a heading (the sections of a document, the dates of the log, the anchors) reads it here.
pub fn heading(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let level = rest.len() - rest.trim_start_matches('#').len();
    let rest = &rest[level..];
    if indent > 3
        || !(1..=6).contains(&level)
        || !(rest.is_empty() || rest.starts_with([' ', '\t']))
    {
        return None;
    }
    let text = rest.trim_matches([' ', '\t']);
    let open = text.trim_end_matches('#');
    let text = if open.is_empty() || open.ends_with([' ', '\t']) {
        open.trim_end_matches([' ', '\t'])
    } else {
        text
    };
    Some((level, text))
}

/// Every heading anchor in a markdown text. A repeated heading gets `-1`, `-2`, ... as on GitHub.
pub fn anchors(text: &str) -> HashSet<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut found = HashSet::new();
    let shown = visible(text);
    for (_, heading) in shown.lines().filter_map(heading) {
        if heading.is_empty() {
            continue;
        }
        let base = slug(heading);
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
/// machine, which no other checkout and no reader on GitHub can follow. A path with `\` fails: only Windows reads it as
/// a separator, so the same link would resolve on one machine and not on another. A link to a `.py` file names a
/// function or class in its text.
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
    if path.contains('\\') {
        return Some(format!("a path with \\; separate with /: {target}"));
    }
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
        let name = text.trim().trim_matches('`').trim();
        // With no name, the pattern below would match any def
        if name.is_empty() {
            return Some(format!(
                "no function or class named in the link text: {target}"
            ));
        }
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
        // GFM lets a fence be indented by up to 3 spaces, and the closing fence by its own amount
        let text = "# Same\n\n## Same\n\n<!--\n## Hidden\n-->\n\n```\n## Code\n```\n\n   ```\n## Indented\n ```\n";
        let expected: HashSet<String> = ["same", "same-1"].map(String::from).into();
        assert_eq!(anchors(text), expected);
    }

    /// GFM's headings: indented by up to 3 spaces, and without the closing run of `#` some write after the text
    #[test]
    fn headings_as_gfm_reads_them() {
        let text = " # One space\n   ## Three spaces\n    # Four spaces is code\n## Closed ##\n#No space\n";
        let expected: HashSet<String> = ["one-space", "three-spaces", "closed"]
            .map(String::from)
            .into();
        assert_eq!(anchors(text), expected);
    }

    /// What GFM hides, and what only looks hidden. Every heading named "Hidden" must give no anchor.
    #[test]
    fn hidden_as_gfm_hides() {
        let text = "\
# Shown

~~~
# Hidden: a tilde fence
~~~

````
```
# Hidden: a shorter fence does not close a longer one
```
````

~~~
```
# Hidden: a fence of the other character does not close it
~~~

``` not a fence, inline code ```

# Shown after inline code

```
<!--
```

# Shown after a comment opener inside code

Text <!-- one line --> and text <!--
# Hidden: a comment that starts mid-line
--> text

# Shown after a comment

```
# Hidden: an unclosed fence runs to the end
";
        let expected: HashSet<String> = [
            "shown",
            "shown-after-inline-code",
            "shown-after-a-comment-opener-inside-code",
            "shown-after-a-comment",
        ]
        .map(String::from)
        .into();
        assert_eq!(anchors(text), expected);
        assert_eq!(visible("a <!-- b --> c\n<!--\nd\n-->\ne\n"), "a  c\n\ne\n");
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
            ("a scheme with a plus", "coap+tcp://example.com/x"),
            ("mail", "mailto:someone@example.com"),
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
            // A file name with a line number is a path, not a URL: no scheme contains a dot
            ("a file and a line", "nothing.md:12"),
            ("an existing file and a line", "rules.md:3"),
            // The file exists, but only Windows reads `\` as a separator
            ("backslashes", "..\\log.md"),
            // A link to a .py file names what it points at; with no name, any def would do
            ("", "../../tests/backlog_bundle.py"),
            ("``", "../../tests/backlog_bundle.py"),
        ];
        let found = reasons(root.path(), &bad);
        for ((text, target), why) in bad.iter().zip(&found) {
            assert!(why.is_some(), "{text} ({target}) passed");
        }
    }

    #[test]
    fn a_backslash_fails_on_every_platform() {
        // Off Windows, `..\log.md` is a missing file anyway; the reason shows it fails for the separator everywhere
        let root = tree();
        let why = reasons(root.path(), &[("x", "..\\log.md")]).remove(0);
        assert!(
            why.as_ref()
                .is_some_and(|why| why.contains("separate with /")),
            "{why:?}"
        );
    }
}
