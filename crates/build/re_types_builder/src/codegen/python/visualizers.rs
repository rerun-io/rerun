use itertools::Itertools as _;

use crate::codegen::autogen_warning;
use crate::codegen::common::StringExt as _;
use crate::codegen::python::quote_init_parameter_from_field;
use crate::{
    ATTR_RERUN_VISUALIZER, ATTR_RERUN_VISUALIZER_NONE, Object, ObjectKind, Objects, Reporter,
};

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
    code.push_unindented("from __future__ import annotations\n\n", 1);

    code.push_indented(0, "from typing import Any, Iterable", 2);
    code.push_indented(0, "from ._base import Visualizer", 2);
    code.push_indented(0, "from ... import components, datatypes", 2);

    let mut visualizers: Vec<(&Object, String)> = Vec::new();
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
            visualizers.push((obj, visualizer_name));
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

    visualizers.sort_by(|a, b| a.0.cmp(b.0));

    // Generated visualizer classes
    for (archetype, visualizer_id) in &visualizers {
        let parameters = archetype
            .fields
            .iter()
            .map(|field| {
                let mut field = field.clone();
                field.is_nullable = true;
                quote_init_parameter_from_field(&field, objects, &archetype.fqname)
            })
            .collect_vec()
            .join(",\n");

        code.push_indented(0, format!("class {}(Visualizer):", archetype.name), 1);
        code.push_indented(
            1,
            format!("def __init__(self, *, {parameters}) -> None:"),
            1,
        );

        code.push_indented(
            2,
            format!("from ...archetypes import {}", archetype.name),
            1,
        );
        code.push_indented(
            2,
            format!(
                "overrides = {}.from_fields({})",
                archetype.name,
                archetype
                    .fields
                    .iter()
                    .map(|field| format!("{}={}", field.name, field.name))
                    .collect_vec()
                    .join(", ")
            ),
            1,
        );

        code.push_indented(
            2,
            format!(
                "super().__init__(\"{visualizer_id}\", overrides=overrides, mappings=None)", // TODO(RR-3254): Support mappings
            ),
            2,
        );
    }

    code
}
