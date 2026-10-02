//! The structure check: the tree agrees with `.config/docket.toml`, either way.
//!
//! - Every layer of the stack's layout is present or declared absent, and no layer declared absent is present.
//! - No code sits outside the layers, except in the stack's paths that are not layers (`tests/`) and the paths the
//!   project lists in `unchecked`. A path in `unchecked` exists and holds no layer, so a layer cannot be switched off by
//!   listing it.
//!
//! The floor: at least one layer is present. A project with every layer declared absent would check nothing. A
//! repository that keeps records only declares `stack = "none"`, and the check says that it skipped the layers.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use crate::layers::{DECLARATION, Declared, Layout, MISSING, declaration};
use crate::source::{exactly, relative_path};

/// Whether `path` is `prefix` or inside it. Both are from the root, with `/`.
fn within(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// What the structure check found.
#[derive(Debug, Default)]
pub struct Structure {
    /// Every way the tree differs from its declaration
    pub found: Vec<String>,
    /// What was not checked, and why. Printed on every run, so a skipped check is never silent
    pub skipped: Option<String>,
}

/// Every way the tree at `root` differs from its declaration. An error is a file or directory that could not be read.
pub fn problems(root: &Path) -> io::Result<Structure> {
    let failed = |found: Vec<String>| {
        Ok(Structure {
            found,
            skipped: None,
        })
    };
    let declared = match declaration(root)? {
        None => return failed(vec![MISSING.into()]),
        Some(Err(why)) => return failed(vec![format!("{DECLARATION}: {why}")]),
        Some(Ok(declaration)) => match Declared::new(declaration) {
            Ok(declared) => declared,
            Err(found) => return failed(found),
        },
    };
    let Some(layout) = &declared.layout else {
        return Ok(Structure {
            found: Vec::new(),
            skipped: Some(format!(
                "the layers are not checked: {DECLARATION} declares stack = \"none\" (records only)"
            )),
        });
    };
    let mut found = Vec::new();
    // By the exact name: `Domain/` is `domain/` on Windows, and a directory of its own on Linux and GitHub
    let present = |path: &str| exactly(root, path).is_ok();

    let mut any = false;
    for place in &declared.places {
        // A level is judged only when its layer is there and not declared absent: otherwise the layer's own finding
        // says it all
        if let Some(parent) = &place.parent {
            let parent = declared.places.iter().find(|p| &p.name == parent).unwrap();
            if declared.is_absent(parent) || !present(&parent.path) {
                continue;
            }
        }
        match (declared.is_absent(place), present(&place.path)) {
            (true, true) => found.push(format!(
                "{} is declared absent, but {}/ exists: remove one or the other",
                place.name, place.path
            )),
            (false, false) => found.push(match exactly(root, &place.path) {
                // A directory in another case: `docket create` would write into it on Windows, and fix nothing
                Err(why) if why.contains(" is there") => format!("{} is {why}", place.name),
                _ => format!(
                    "{} is missing: {}/ (run `docket create`, or declare it in absent in {DECLARATION})",
                    place.name, place.path
                ),
            }),
            (false, true) => any = true,
            (true, false) => {}
        }
    }
    if !any {
        found.push(format!(
            "no layer is present: the {} layout has {}",
            declared.declaration.stack,
            declared
                .places
                .iter()
                .filter(|p| p.parent.is_none())
                .map(|p| format!("{}/", p.path))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let mut skipped: Vec<&str> = layout.not_layers.iter().map(String::as_str).collect();
    for path in &declared.declaration.unchecked {
        let path = path.trim_end_matches('/');
        if path.contains('\\') {
            found.push(format!(
                "unchecked lists {path:?}: separate a path with /, which every platform reads"
            ));
        } else if path.starts_with('/')
            || path.contains(':')
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            // `./scripts` would never equal the paths docket compares it with, and would switch nothing off silently
            found.push(format!(
                "unchecked lists {path:?}: write a path from the root, such as scripts or src/generated"
            ));
        } else if let Some(place) = declared
            .places
            .iter()
            .find(|p| within(&p.path, path) || within(path, &p.path))
        {
            found.push(format!(
                "unchecked lists {path}, which holds or sits in the layer {} ({}/): a layer cannot be switched off",
                place.name, place.path
            ));
        } else if !present(path) {
            found.push(format!(
                "unchecked lists {path}, which does not exist: remove it"
            ));
        } else {
            skipped.push(path);
        }
    }
    let places: Vec<&str> = declared.places.iter().map(|p| p.path.as_str()).collect();
    for outside in outside(root, layout, &places, &skipped)? {
        found.push(format!(
            "code outside the layers: {outside} (move it into a layer, or list it in unchecked in {DECLARATION})"
        ));
    }
    Ok(Structure {
        found,
        skipped: None,
    })
}

/// The entries of the layout's scope that hold code outside `places` and `skipped`: a file, or the directory directly
/// in the scope that holds it. Files the project's `.gitignore` files exclude, and hidden ones, are not looked at.
///
/// Only the `.gitignore` files inside the project count. A global excludes file, `.git/info/exclude` and a
/// `.gitignore` above the root differ from one machine to another, and would hide code on one machine that fails on
/// another.
fn outside(
    root: &Path,
    layout: &Layout,
    places: &[&str],
    skipped: &[&str],
) -> io::Result<BTreeSet<String>> {
    let scope = &layout.scope;
    let mut found = BTreeSet::new();
    if !exactly(root, scope).is_ok_and(|path| path.is_dir()) {
        return Ok(found);
    }
    let walk = ignore::WalkBuilder::new(root)
        .require_git(false)
        .parents(false)
        .ignore(false)
        .git_global(false)
        .git_exclude(false)
        // From the root, so its `.gitignore` applies to the scope too; into the scope only
        .filter_entry({
            let root = root.to_path_buf();
            let scope = scope.clone();
            move |entry| {
                let path = relative_path(entry.path(), &root);
                scope.is_empty()
                    || path.is_empty()
                    || within(&path, &scope)
                    || within(&scope, &path)
            }
        })
        .build();
    for entry in walk {
        let entry = entry.map_err(|e| io::Error::other(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file())
            || !layout.is_code(&entry.file_name().to_string_lossy())
        {
            continue;
        }
        let path = relative_path(entry.path(), root);
        if places.iter().chain(skipped).any(|p| within(&path, p)) {
            continue;
        }
        let inside = if scope.is_empty() {
            path.as_str()
        } else {
            &path[scope.len() + 1..]
        };
        let first = inside.split('/').next().unwrap_or(inside);
        found.insert(if scope.is_empty() {
            first.to_string()
        } else {
            format!("{scope}/{first}")
        });
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_compares_whole_names() {
        assert!(within("domain", "domain"));
        assert!(within("domain/x.py", "domain"));
        assert!(!within("domains/x.py", "domain"));
        assert!(!within("domain", "domain/x.py"));
    }
}
