//! `docket create`: make what `.config/docket.toml` declares and the tree does not have yet.
//!
//! - Each layer of the stack's layout that is neither declared absent nor present (its path exists) is made, with its
//!   files. A present layer is the project's, and nothing in it is touched.
//!   A repository that keeps records only (`stack = "none"`) has no layers to make.
//! - The records skeleton: the directories of `docs/`, `docs/log.md` with its title when it does not exist, and the
//!   generated files (the index files and `docs/backlog/rules.md`), which docket rewrites.
//!
//! It never overwrites a file it does not generate, and never moves or deletes one. It runs when a project starts, and
//! again when its declaration is changed on purpose; it never runs by itself, so a layer removed by mistake fails the
//! check instead of coming back.

use std::fs;
use std::path::Path;

use crate::bundle::{Bundle, LOG};
use crate::layers::{DECLARATION, Declared, MISSING, declaration};
use crate::source::{read_source, relative_path};

/// What `docket create` did.
#[derive(Debug, Default)]
pub struct Made {
    /// The files written, from the root, with `/`: made new or rewritten with a change
    pub written: Vec<String>,
    /// Documents left out of the index files: name -> why
    pub left_out: Vec<(String, String)>,
}

/// Make what is missing at `root`. `Err` is a declaration that cannot be read, or a file that cannot be written.
pub fn create(root: &Path) -> Result<Made, String> {
    let declared = match declaration(root).map_err(|e| e.to_string())? {
        None => return Err(MISSING.into()),
        Some(Err(why)) => return Err(format!("{DECLARATION}: {why}")),
        Some(Ok(declaration)) => Declared::new(declaration).map_err(|found| found.join("\n"))?,
    };
    let mut made = Made::default();
    for place in &declared.places {
        // A level whose layer is declared absent is absent too. Otherwise its layer was made just before it
        if declared.is_absent(place) || root.join(&place.path).exists() {
            continue;
        }
        for (path, text) in &place.files {
            write(root, path, text, &mut made)?;
        }
    }

    let bundle = Bundle::new(root);
    for folder in ["backlog", "specs", "done"] {
        let dir = bundle.docs.join(folder);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if !bundle.docs.join("log.md").exists() {
        write(root, "docs/log.md", LOG, &mut made)?;
    }
    let (files, problems) = bundle.expected().map_err(|e| e.to_string())?;
    for (path, text) in files {
        if read_source(&path).ok().as_ref() != Some(&text) {
            write(root, &relative_path(&path, root), &text, &mut made)?;
        }
    }
    made.left_out = problems.into_iter().collect();
    Ok(made)
}

fn write(root: &Path, path: &str, text: &str, made: &mut Made) -> Result<(), String> {
    let full = root.join(path);
    if let Some(dir) = full.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::write(&full, text).map_err(|e| format!("{path}: {e}"))?;
    made.written.push(path.to_string());
    Ok(())
}
