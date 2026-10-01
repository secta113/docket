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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings_become_lf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "# A\r\nb\rc\n").unwrap();
        assert_eq!(read_source(&path).unwrap(), "# A\nb\nc\n");
    }
}
