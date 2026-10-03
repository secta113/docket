//! The files Rotproof writes outside `docs/`: its own guide, `.rotproof/AGENTS.md`, which every `rotproof create`
//! rewrites and `rotproof check` compares, as it does the generated files in `docs/`.
//!
//! The guide says the rules Rotproof keeps in the project's stack: how to run Rotproof, the layers (their table is
//! written from `layers/table.toml`, so the two cannot disagree) and the records. It names the version that wrote it,
//! so an upgrade fails `rotproof check` until `rotproof create` has run.
//!
//! The texts are in `project/`, built into the binary.

use crate::layers::{Layout, listed, table};

/// Rotproof's guide, from the root
pub const GUIDE: &str = ".rotproof/AGENTS.md";

const GUIDE_TEXT: &str = include_str!("../project/rotproof/AGENTS.md");
const LAYERS_TEXT: &str = include_str!("../project/rotproof/layers.md");
const UI_TEXT: &str = include_str!("../project/rotproof/ui.md");

/// The guide for a project of `stack`, whose layout is `layout` (`None` for a repository of records only).
pub fn guide(stack: &str, layout: Option<&Layout>) -> String {
    let (kept, checked, layers) = match layout {
        None => (
            format!(
                "Rotproof keeps the records in `docs/` in this project. It has no layers: its stack is \"{stack}\"."
            ),
            "the records",
            String::new(),
        ),
        Some(layout) => (
            "Rotproof keeps two things in this project: the layers (where code lives and what it may import) and the\n\
             records in `docs/`."
                .to_string(),
            "the layers and the records",
            layers_section(layout),
        ),
    };
    GUIDE_TEXT
        .replace("{version}", env!("CARGO_PKG_VERSION"))
        .replace("{stack}", stack)
        .replace("{kept}", &kept)
        .replace("{checked}", checked)
        .replace("{layers}", &layers)
}

/// The section on the layers: the rules, and the table of where each layer lives in this stack and what it may import.
fn layers_section(layout: &Layout) -> String {
    let table = table();
    let has = |name: &String| !layout.without.contains(name);
    let mut rows = vec![
        "| Layer | Where | May import |".to_string(),
        "|---|---|---|".to_string(),
    ];
    for layer in table.layers.iter().filter(|l| has(&l.name)) {
        let path = layout.layer.path.replace("{name}", &layer.name);
        let imports: Vec<String> = layer.imports.iter().filter(|n| has(n)).cloned().collect();
        let imports = if layer.name == "ui" {
            "Nothing: it holds only its levels".to_string()
        } else if imports.is_empty() {
            "No other layer".to_string()
        } else {
            listed(&imports)
        };
        rows.push(format!("| `{}` | `{path}/` | {imports} |", layer.name));
        if layer.name != "ui" {
            continue;
        }
        let level = layout
            .level
            .as_ref()
            .expect("a layout with ui has its levels: a test reads every layout");
        for (i, entry) in table.levels.iter().enumerate() {
            let path = level.path.replace("{name}", &entry.name);
            let below = if i + 1 < table.levels.len() {
                "The levels below it, and "
            } else {
                ""
            };
            rows.push(format!(
                "| `ui.{}` | `{path}/` | {below}{} |",
                entry.name,
                listed(&entry.imports)
            ));
        }
    }
    let not_layers = if layout.not_layers.is_empty() {
        String::new()
    } else {
        // Paths from the root, as `unchecked` writes them, on a line of their own in the list item
        format!(
            "\n  {} {}, and may hold code.",
            listed(&layout.not_layers),
            if layout.not_layers.len() == 1 {
                "is not a layer"
            } else {
                "are not layers"
            }
        )
    };
    let ui = if has(&"ui".to_string()) { UI_TEXT } else { "" };
    LAYERS_TEXT
        .replace("{not_layers}", &not_layers)
        .replace("{table}", &format!("{}\n", rows.join("\n")))
        .replace("{ui}", ui)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::{RECORDS_ONLY, STACKS, layout};

    #[test]
    fn every_stack_has_a_guide_with_every_placeholder_filled() {
        let mut guides: Vec<String> = STACKS
            .iter()
            .map(|(stack, _)| guide(stack, layout(stack).as_ref()))
            .collect();
        guides.push(guide(RECORDS_ONLY, None));
        for text in &guides {
            for placeholder in [
                "{version}",
                "{stack}",
                "{kept}",
                "{checked}",
                "{layers}",
                "{not_layers}",
                "{table}",
                "{ui}",
            ] {
                assert!(
                    !text.contains(placeholder),
                    "{placeholder} left in:\n{text}"
                );
            }
            assert!(text.contains(env!("CARGO_PKG_VERSION")));
        }
    }

    #[test]
    fn the_table_lists_every_place_of_the_layout() {
        for (stack, _) in STACKS {
            let layout = layout(stack).unwrap();
            let text = guide(stack, Some(&layout));
            let places = layout.places(&table());
            assert!(!places.is_empty());
            for place in places {
                let row = format!("| `{}` | `{}/` |", place.name, place.path);
                assert!(text.contains(&row), "{stack}: no row {row}");
            }
            assert_eq!(
                text.contains("atomic design"),
                !layout.without.contains(&"ui".to_string()),
                "{stack}: the rule on ui goes with the layer"
            );
        }
    }

    #[test]
    fn a_repository_of_records_only_has_no_section_on_layers() {
        let text = guide(RECORDS_ONLY, None);
        assert!(!text.contains("## The layers"));
        assert!(text.contains("## The records"));
    }
}
