use crate::codegen::autogen_warning;
use crate::codegen::common::StringExt as _;
use crate::{ATTR_RERUN_VISUALIZER, ATTR_RERUN_VISUALIZER_NONE, ObjectKind, Objects, Reporter};

/// Generate the `visualizers.py` file containing constants for visualizer identifiers.
///
/// This function iterates through all archetypes and includes those that have the
/// `attr.rerun.visualizer` attribute set with a value. Archetypes with
/// `attr.rerun.visualizer_none` are explicitly skipped.
///
/// The generated constants are sorted alphabetically by archetype name.
pub fn generate_visualizers_file(reporter: &Reporter, objects: &Objects) -> String {
    let mut code = String::new();

    code.push_indented(0, format!("# {}", autogen_warning!()), 3);
    code.push_indented(
        0,
        "\"\"\"Constants for the names of the visualizers.\"\"\"",
        2,
    );
    code.push_unindented("from __future__ import annotations\n\n", 0);

    code.push_indented(0, "from typing import Any", 2);

    let mut visualizers: Vec<(String, String)> = Vec::new();
    let mut archetypes_without_attr = Vec::new();

    for obj in objects
        .objects_of_kind(ObjectKind::Archetype)
        .filter(|obj| !obj.is_testing())
    {
        // Skip blueprint archetypes - they don't have visualizers
        if obj.scope() == Some("blueprint".into()) {
            continue;
        }

        if obj.is_attr_set(ATTR_RERUN_VISUALIZER_NONE) {
            continue;
        }

        if let Some(visualizer_name) = obj.try_get_attr::<String>(ATTR_RERUN_VISUALIZER) {
            visualizers.push((obj.name.clone(), visualizer_name));
        } else {
            archetypes_without_attr.push(obj.name.clone());
        }
    }

    if !archetypes_without_attr.is_empty() {
        reporter.error(
            "visualizers.py codegen",
            "",
            format!(
                "The following archetypes are missing both '{}' and '{}' attributes:\n  - {}",
                ATTR_RERUN_VISUALIZER,
                ATTR_RERUN_VISUALIZER_NONE,
                archetypes_without_attr.join("\n  - ")
            ),
        );
    }

    visualizers.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate string constants
    for (archetype_name, visualizer_id) in &visualizers {
        code.push_indented(0, format!("{archetype_name} = {visualizer_id:?}"), 1);
    }

    // TODO(RR-3173): This should not be experimental anymore.
    // Generate experimental module with visualizer classes
    code.push_unindented("\n\n# Experimental API for configuring visualizers", 1);
    code.push_indented(0, "class experimental:", 1);

    code.push_indented(1, "from ..experimental import Visualizer", 2);

    // Generated visualizer classes
    for (archetype_name, visualizer_id) in &visualizers {
        code.push_indented(1, format!("class {archetype_name}(Visualizer):"), 1);
        code.push_indented(
            2,
            "def __init__(self, *, overrides: Any = None, mappings: Any = None) -> None:",
            1,
        );
        code.push_indented(
            3,
            format!(
                "super().__init__(\"{visualizer_id}\", overrides=overrides, mappings=mappings)",
            ),
            2,
        );
    }

    code
}
