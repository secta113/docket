//! Reading the files the checks look at.

use std::fs;
use std::io;
use std::path::Path;

/// Read a file as UTF-8, with every line ending as `\n`.
///
/// Line endings are normalized as Python's text mode does, which the original checks read with: a file checked out with
/// CRLF would otherwise keep `\r` at the end of each heading, and its anchors would differ.
pub fn read_source(path: &Path) -> io::Result<String> {
    Ok(fs::read_to_string(path)?
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}

/// The path from the root, for messages: with `/` on every platform, so the output is the same on Windows.
pub fn relative_path(path: &Path, root: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_uses_slashes() {
        let root = Path::new("repo");
        assert_eq!(
            relative_path(&root.join("docs").join("backlog").join("index.md"), root),
            "docs/backlog/index.md"
        );
    }

    #[test]
    fn line_endings_become_lf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "# A\r\nb\rc\n").unwrap();
        assert_eq!(read_source(&path).unwrap(), "# A\nb\nc\n");
    }
}
