//! `rotproof init`: write a project's declaration, `.config/rotproof.toml`, once.
//!
//! It writes only the declaration, so the project declares the layers it does not have before `rotproof create` makes
//! anything. The declaration is the project's from then on: `rotproof init` never overwrites it.

use std::fs;
use std::path::Path;

use crate::layers::{DECLARATION, declaration_text, known_stacks};

/// Write the declaration for `stack` at `root`, and return its path. `Err` when the stack is unknown, the declaration
/// exists, or it cannot be written.
pub fn init(root: &Path, stack: &str) -> Result<&'static str, String> {
    if !known_stacks().contains(&stack) {
        return Err(format!(
            "unknown stack {stack:?} (known: {})",
            known_stacks().join(", ")
        ));
    }
    let path = root.join(DECLARATION);
    if path.exists() {
        return Err(format!(
            "{DECLARATION} exists, and rotproof init never overwrites it: edit it, then run `rotproof create`"
        ));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::write(&path, declaration_text(stack)).map_err(|e| format!("{DECLARATION}: {e}"))?;
    Ok(DECLARATION)
}
