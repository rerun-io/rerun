use std::collections::{BTreeMap, BTreeSet};

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::archetypes as bp;
use re_sdk_types::blueprint::components::{
    ActiveTab, IncludedContent, RootContainer, ViewMaximized, VisualizerInstructionId,
};
use re_sdk_types::external::uuid::{Uuid, Variant};
use re_viewer_context::{ContainerId, ViewId};

/// A blueprint entity that only shows when something refers to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Referenced {
    /// A `ViewBlueprint` under `/view`.
    View,

    /// A `ContainerBlueprint` under `/container`.
    Container,

    /// A `VisualizerInstruction`, listed by the `ActiveVisualizers` of its parent entity.
    VisualizerInstruction,
}

/// A reference between blueprint entities that the viewer cannot follow.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceProblem {
    /// `field` of `source` refers to `target`, which is not a `expected`.
    Dead {
        /// The entity holding the reference.
        source: EntityPath,

        /// The referencing field, as `Archetype.field`.
        field: &'static str,

        /// The entity the reference names.
        target: EntityPath,

        /// The archetypes `target` must hold.
        expected: &'static str,
    },

    /// Nothing refers to `entity`, so the viewer never shows it.
    Orphan {
        /// The unreferenced entity.
        entity: EntityPath,

        /// What `entity` is.
        kind: Referenced,
    },

    /// Containers that hold each other in a loop, starting at the smallest path.
    Cycle {
        /// The containers in the loop, each in the `contents` of the previous one.
        containers: Vec<EntityPath>,
    },

    /// `child` is in the `contents` of more than one container, or more than once in one.
    SharedChild {
        /// The view or container listed more than once.
        child: EntityPath,

        /// Each container listing `child`, once per listing.
        parents: Vec<EntityPath>,
    },
}

impl std::fmt::Display for ReferenceProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dead {
                source,
                field,
                target,
                expected,
            } => {
                write!(
                    f,
                    "`{field}` of {source} refers to {target}, which has no {expected}. "
                )?;
                let write_it = |f: &mut std::fmt::Formatter<'_>| {
                    write!(
                        f,
                        "Write a {expected} at {target} in the same call, or remove the reference."
                    )
                };
                if *field == "ActiveVisualizers.instruction_ids" {
                    return write_it(f);
                }
                let registry = target.parent().filter(|parent| {
                    parent == ViewId::registry() || parent == ContainerId::registry()
                });
                let uuid = target
                    .last()
                    .and_then(|last| Uuid::parse_str(last.unescaped_str()).ok());
                match (registry, uuid) {
                    (Some(registry), Some(uuid)) => {
                        let canonical =
                            registry.join(&EntityPath::from_single_string(uuid.to_string()));
                        if canonical == *target {
                            write_it(f)
                        } else {
                            write!(
                                f,
                                "Write it as {canonical}: the viewer only finds lowercase, \
                                 hyphenated UUIDs."
                            )
                        }
                    }
                    _ if *expected == "ViewBlueprint" => {
                        f.write_str("Refer to a `/view/<uuid>` path instead.")
                    }
                    _ => f.write_str(
                        "Refer to a `/view/<uuid>` or `/container/<uuid>` path instead.",
                    ),
                }
            }
            Self::Orphan { entity, kind } => {
                let clear = serde_json::json!({
                    entity.to_string(): {"Clear": {"is_recursive": true}}
                });
                match kind {
                    Referenced::View | Referenced::Container => write!(
                        f,
                        "{entity} is not in the `contents` of any container reachable from \
                         `root_container`, nor in a table preview, so it never shows. \
                         Add it to `contents` in the same call, or delete it with `{clear}`."
                    ),
                    Referenced::VisualizerInstruction => {
                        let parent = entity.parent().unwrap_or_else(EntityPath::root);
                        write!(
                            f,
                            "{entity} is not in `instruction_ids` of the `ActiveVisualizers` at \
                             {parent}, so it is never used. Add its id there in the same call, \
                             or delete it with `{clear}`."
                        )
                    }
                }
            }
            Self::Cycle { containers } => {
                let chain = std::iter::chain(containers, containers.first())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" → ");
                write!(
                    f,
                    "Containers hold each other in a loop: {chain}. \
                     Remove one of them from the `contents` of the one before it."
                )
            }
            Self::SharedChild { child, parents } => {
                let parents = parents
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "{child} is listed in `contents` more than once, by {parents}. \
                     A view or container has exactly one parent."
                )
            }
        }
    }
}

/// Every reference in `blueprint`, as seen by `query`, that the viewer cannot follow, sorted.
///
/// Honors `Clear`s, as the viewer does: a cleared view or container no longer exists.
/// Views and containers only count at their canonical path, `/view/<uuid>` or
/// `/container/<uuid>` with a lowercase, hyphenated UUID, since that is where the viewer looks.
pub fn reference_problems(blueprint: &EntityDb, query: &LatestAtQuery) -> Vec<ReferenceProblem> {
    re_tracing::profile_function!();

    let latest_at = |entity: &EntityPath, component: ComponentIdentifier| {
        blueprint.latest_at(query, entity, [component])
    };
    let has = |entity: &EntityPath, component: ComponentIdentifier| {
        latest_at(entity, component)
            .component_batch_raw(component)
            .is_some_and(|batch| !batch.is_empty())
    };
    let paths = |entity: &EntityPath, component: ComponentIdentifier| -> Vec<EntityPath> {
        latest_at(entity, component)
            .component_batch_quiet::<IncludedContent>(component)
            .unwrap_or_default()
            .into_iter()
            .map(|content| content.0.into())
            .collect()
    };

    let entities: BTreeSet<EntityPath> = blueprint.storage_engine().store().all_entities_sorted();
    let views: BTreeSet<EntityPath> = entities
        .iter()
        .filter(|entity| {
            ViewId::from_entity_path(entity).as_entity_path() == **entity
                && has(
                    entity,
                    bp::ViewBlueprint::descriptor_class_identifier().component,
                )
        })
        .cloned()
        .collect();
    let containers: BTreeSet<EntityPath> = entities
        .iter()
        .filter(|entity| {
            ContainerId::from_entity_path(entity).as_entity_path() == **entity
                && has(
                    entity,
                    bp::ContainerBlueprint::descriptor_container_kind().component,
                )
        })
        .cloned()
        .collect();
    let instructions: BTreeSet<EntityPath> = entities
        .iter()
        .filter(|entity| {
            has(
                entity,
                bp::VisualizerInstruction::descriptor_visualizer_type().component,
            )
        })
        .cloned()
        .collect();

    let mut problems = BTreeSet::new();
    let mut check_content = |source: &EntityPath, field: &'static str, target: &EntityPath| {
        let (exists, expected) = if target.starts_with(ViewId::registry()) {
            (views.contains(target), "ViewBlueprint")
        } else if target.starts_with(ContainerId::registry()) {
            (containers.contains(target), "ContainerBlueprint")
        } else {
            (false, "ViewBlueprint or ContainerBlueprint")
        };
        if !exists {
            problems.insert(ReferenceProblem::Dead {
                source: source.clone(),
                field,
                target: target.clone(),
                expected,
            });
        }
    };

    let viewport = EntityPath::from("/viewport");
    let root = latest_at(
        &viewport,
        bp::ViewportBlueprint::descriptor_root_container().component,
    )
    .component_mono_quiet::<RootContainer>(
        bp::ViewportBlueprint::descriptor_root_container().component,
    )
    .map(|root| ContainerId::from(root.0).as_entity_path());
    if let Some(root) = &root {
        check_content(&viewport, "ViewportBlueprint.root_container", root);
    }
    if let Some(maximized) = latest_at(
        &viewport,
        bp::ViewportBlueprint::descriptor_maximized().component,
    )
    .component_mono_quiet::<ViewMaximized>(bp::ViewportBlueprint::descriptor_maximized().component)
    {
        check_content(
            &viewport,
            "ViewportBlueprint.maximized",
            &ViewId::from(maximized.0).as_entity_path(),
        );
    }

    let mut children: BTreeMap<&EntityPath, Vec<EntityPath>> = BTreeMap::new();
    let mut parents: BTreeMap<EntityPath, Vec<EntityPath>> = BTreeMap::new();
    for container in &containers {
        let contents = paths(
            container,
            bp::ContainerBlueprint::descriptor_contents().component,
        );
        for child in &contents {
            check_content(container, "ContainerBlueprint.contents", child);
            parents
                .entry(child.clone())
                .or_default()
                .push(container.clone());
        }
        children.insert(container, contents);

        let active_tab = bp::ContainerBlueprint::descriptor_active_tab().component;
        if let Some(tab) =
            latest_at(container, active_tab).component_mono_quiet::<ActiveTab>(active_tab)
        {
            check_content(container, "ContainerBlueprint.active_tab", &tab.0.into());
        }
    }

    let mut previewed = Vec::new();
    for entity in &entities {
        for view in paths(entity, bp::TableColumnPreview::descriptor_views().component) {
            if views.contains(&view) {
                previewed.push(view);
            } else {
                problems.insert(ReferenceProblem::Dead {
                    source: entity.clone(),
                    field: "TableColumnPreview.views",
                    target: view,
                    expected: "ViewBlueprint",
                });
            }
        }
    }

    let instruction_ids = bp::ActiveVisualizers::descriptor_instruction_ids().component;
    let mut listed_instructions = BTreeSet::new();
    let mut explicit_lists = BTreeSet::new();
    for entity in &entities {
        let Some(ids) = latest_at(entity, instruction_ids)
            .component_batch_quiet::<VisualizerInstructionId>(instruction_ids)
        else {
            continue;
        };
        explicit_lists.insert(entity);
        for id in ids {
            let instruction = entity.join(&EntityPath::from_single_string(id.to_string()));
            if !instructions.contains(&instruction) {
                problems.insert(ReferenceProblem::Dead {
                    source: entity.clone(),
                    field: "ActiveVisualizers.instruction_ids",
                    target: instruction.clone(),
                    expected: "VisualizerInstruction",
                });
            }
            listed_instructions.insert(instruction);
        }
    }

    for (child, parents) in parents {
        if 1 < parents.len() {
            problems.insert(ReferenceProblem::SharedChild { child, parents });
        }
    }

    for cycle in container_cycles(&children) {
        problems.insert(ReferenceProblem::Cycle { containers: cycle });
    }

    let mut reachable: BTreeSet<EntityPath> = previewed.into_iter().collect();
    let mut to_visit: Vec<EntityPath> = root.into_iter().collect();
    while let Some(entity) = to_visit.pop() {
        if reachable.insert(entity.clone())
            && let Some(contents) = children.get(&entity)
        {
            to_visit.extend(contents.iter().cloned());
        }
    }
    let unreachable = std::iter::chain(
        views.iter().map(|view| (view, Referenced::View)),
        containers
            .iter()
            .map(|container| (container, Referenced::Container)),
    )
    .filter(|(entity, _)| !reachable.contains(*entity));
    // Without an explicit list, the viewer only reuses ids it made itself, whose low half is an
    // index into its recommended visualizers. That leaves the RFC 4122 variant bits unset, so
    // any RFC 4122 UUID, such as one an agent wrote, is never picked.
    let picked_without_list = |instruction: &EntityPath| {
        instruction
            .last()
            .and_then(|last| Uuid::parse_str(last.unescaped_str()).ok())
            .is_some_and(|uuid| uuid.get_variant() != Variant::RFC4122)
    };
    let unlisted = instructions
        .iter()
        .filter(|instruction| {
            let has_list = instruction
                .parent()
                .is_some_and(|parent| explicit_lists.contains(&parent));
            if has_list {
                !listed_instructions.contains(*instruction)
            } else {
                !picked_without_list(instruction)
            }
        })
        .map(|instruction| (instruction, Referenced::VisualizerInstruction));
    for (entity, kind) in std::iter::chain(unreachable, unlisted) {
        problems.insert(ReferenceProblem::Orphan {
            entity: entity.clone(),
            kind,
        });
    }

    problems.into_iter().collect()
}

/// Each loop of containers in `children`, rotated to start at its smallest path.
fn container_cycles(
    children: &BTreeMap<&EntityPath, Vec<EntityPath>>,
) -> BTreeSet<Vec<EntityPath>> {
    #[derive(Clone, Copy)]
    enum Visit {
        InProgress,
        Done,
    }

    fn visit<'a>(
        container: &'a EntityPath,
        children: &'a BTreeMap<&EntityPath, Vec<EntityPath>>,
        visits: &mut BTreeMap<&'a EntityPath, Visit>,
        stack: &mut Vec<&'a EntityPath>,
        cycles: &mut BTreeSet<Vec<EntityPath>>,
    ) {
        visits.insert(container, Visit::InProgress);
        stack.push(container);
        for child in children.get(container).into_iter().flatten() {
            if !children.contains_key(child) {
                continue;
            }
            match visits.get(child) {
                Some(Visit::InProgress) => {
                    if let Some(start) = stack.iter().position(|entity| *entity == child) {
                        let mut cycle: Vec<EntityPath> = stack[start..]
                            .iter()
                            .map(|entity| (*entity).clone())
                            .collect();
                        if let Some(smallest) = (0..cycle.len()).min_by_key(|&i| &cycle[i]) {
                            cycle.rotate_left(smallest);
                        }
                        cycles.insert(cycle);
                    }
                }
                Some(Visit::Done) => {}
                None => visit(child, children, visits, stack, cycles),
            }
        }
        stack.pop();
        visits.insert(container, Visit::Done);
    }

    let mut visits = BTreeMap::new();
    let mut cycles = BTreeSet::new();
    for container in children.keys() {
        if !visits.contains_key(container) {
            visit(
                container,
                children,
                &mut visits,
                &mut Vec::new(),
                &mut cycles,
            );
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_log_types::{StoreId, StoreKind};
    use serde_json::{Value, json};

    use super::*;

    const ROOT: &str = "00000000-0000-4000-8000-000000000001";
    const INNER: &str = "00000000-0000-4000-8000-000000000002";
    const VIEW: &str = "00000000-0000-4000-8000-000000000003";
    const INSTRUCTION: &str = "00000000-0000-4000-8000-000000000004";

    fn container(id: &str) -> String {
        format!("/container/{id}")
    }

    fn view(id: &str) -> String {
        format!("/view/{id}")
    }

    fn write(blueprint: &mut EntityDb, json: &Value) {
        let timepoint = re_viewer_context::blueprint_timepoint_for_writes(blueprint);
        for chunk in re_arrow_json::partial_chunks_from_json(
            json,
            &timepoint,
            re_sdk_types::reflection::reflection(),
        )
        .unwrap()
        {
            blueprint.add_chunk(&Arc::new(chunk)).unwrap();
        }
    }

    fn blueprint(json: &Value) -> EntityDb {
        let mut blueprint = EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        write(&mut blueprint, json);
        blueprint
    }

    fn latest() -> LatestAtQuery {
        LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
    }

    fn problems(blueprint: &EntityDb) -> Vec<String> {
        reference_problems(blueprint, &latest())
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// A root holding a view, whose one entity has a single visualizer instruction.
    fn valid() -> Value {
        let visualizers = format!("{}/ViewContents/overrides/points/visualizers", view(VIEW));
        json!({
            "/viewport": {"ViewportBlueprint": {"root_container": ROOT}},
            container(ROOT): {"ContainerBlueprint": {"container_kind": "Tabs", "contents": [view(VIEW)], "active_tab": view(VIEW)}},
            view(VIEW): {"ViewBlueprint": {"class_identifier": "3D"}},
            visualizers.clone(): {"ActiveVisualizers": {"instruction_ids": [INSTRUCTION]}},
            format!("{visualizers}/{INSTRUCTION}"): {"VisualizerInstruction": {"visualizer_type": "Points3D"}},
        })
    }

    /// Every blueprint archetype field that holds a UUID or an entity path must be listed here,
    /// so that adding one to the schema fails this test until `reference_problems` handles it.
    #[test]
    fn every_reference_field_in_the_schema_is_checked() {
        // Checked by `reference_problems`.
        const BLUEPRINT_REFERENCES: &[&str] = &[
            "ActiveVisualizers.instruction_ids",
            "ContainerBlueprint.active_tab",
            "ContainerBlueprint.contents",
            "TableColumnPreview.views",
            "ViewportBlueprint.maximized",
            "ViewportBlueprint.root_container",
        ];
        // These name entities of the recording, not of the blueprint.
        const RECORDING_REFERENCES: &[&str] = &[
            "EyeControls3D.tracking_entity",
            "ViewBlueprint.space_origin",
        ];

        let reflection = re_sdk_types::reflection::reflection();
        let mut reference_fields = vec![];
        #[expect(clippy::iter_over_hash_type)] // Sorted below.
        for (archetype, archetype_reflection) in &reflection.archetypes {
            if !archetype
                .full_name()
                .starts_with("rerun.blueprint.archetypes.")
            {
                continue;
            }
            for field in &archetype_reflection.fields {
                let encoding = reflection
                    .components
                    .get(&field.component_type)
                    .and_then(|component| component.encoding);
                if matches!(
                    encoding,
                    Some("rerun.encodings.Uuid" | "rerun.encodings.EntityPath")
                ) {
                    reference_fields.push(format!("{}.{}", archetype.short_name(), field.name));
                }
            }
        }
        reference_fields.sort();

        let mut listed: Vec<String> = std::iter::chain(BLUEPRINT_REFERENCES, RECORDING_REFERENCES)
            .map(ToString::to_string)
            .collect();
        listed.sort();
        assert_eq!(reference_fields, listed);
    }

    #[test]
    fn a_tree_has_no_problems() {
        assert!(problems(&blueprint(&valid())).is_empty());
    }

    #[test]
    fn every_kind_of_problem_names_where_it_is() {
        let visualizers = format!("{}/visualizers", view(VIEW));
        let blueprint = blueprint(&json!({
            "/viewport": {"ViewportBlueprint": {"root_container": ROOT, "maximized": INNER}},
            container(ROOT): {"ContainerBlueprint": {"container_kind": "Horizontal", "contents": [container(INNER), view(VIEW), view(VIEW), "/world/points"]}},
            container(INNER): {"ContainerBlueprint": {"container_kind": "Vertical", "contents": [container(ROOT)]}},
            view(VIEW): {"ViewBlueprint": {"class_identifier": "3D"}},
            view(INSTRUCTION): {"ViewBlueprint": {"class_identifier": "3D"}},
            visualizers.clone(): {"ActiveVisualizers": {"instruction_ids": [INSTRUCTION]}},
            format!("{visualizers}/{ROOT}"): {"VisualizerInstruction": {"visualizer_type": "Points3D"}},
        }));
        insta::assert_snapshot!(problems(&blueprint).join("\n"));
    }

    #[test]
    fn a_cleared_view_no_longer_exists() {
        let mut blueprint = blueprint(&valid());
        write(
            &mut blueprint,
            &json!({view(VIEW): {"Clear": {"is_recursive": true}}}),
        );
        assert_eq!(
            reference_problems(&blueprint, &latest()),
            [
                ReferenceProblem::Dead {
                    source: container(ROOT).into(),
                    field: "ContainerBlueprint.active_tab",
                    target: view(VIEW).into(),
                    expected: "ViewBlueprint",
                },
                ReferenceProblem::Dead {
                    source: container(ROOT).into(),
                    field: "ContainerBlueprint.contents",
                    target: view(VIEW).into(),
                    expected: "ViewBlueprint",
                },
            ]
        );
    }

    #[test]
    fn without_an_explicit_list_only_viewer_made_instructions_are_used() {
        let visualizers = format!("{}/ViewContents/overrides/points/visualizers", view(VIEW));
        let viewer_made =
            re_sdk_types::blueprint::components::VisualizerInstructionId::new_deterministic(
                &"/points".into(),
                0,
            );
        let mut json = valid();
        let entities = json.as_object_mut().unwrap();
        entities.remove(&visualizers);
        entities.insert(
            format!("{visualizers}/{viewer_made}"),
            json!({"VisualizerInstruction": {"visualizer_type": "Points3D"}}),
        );
        assert_eq!(
            reference_problems(&blueprint(&json), &latest()),
            [ReferenceProblem::Orphan {
                entity: format!("{visualizers}/{INSTRUCTION}").into(),
                kind: Referenced::VisualizerInstruction,
            }]
        );
    }

    #[test]
    fn a_table_preview_shows_only_views() {
        let blueprint = blueprint(&json!({
            "/table/layouts/cards/fields/link": {"TableColumnPreview": {"views": container(ROOT)}},
            container(ROOT): {"ContainerBlueprint": {"container_kind": "Tabs"}},
        }));
        assert_eq!(
            reference_problems(&blueprint, &latest())[0],
            ReferenceProblem::Dead {
                source: "/table/layouts/cards/fields/link".into(),
                field: "TableColumnPreview.views",
                target: container(ROOT).into(),
                expected: "ViewBlueprint",
            }
        );
    }

    #[test]
    fn a_container_only_counts_at_its_lowercase_uuid() {
        let root = "abcdef00-0000-4000-8000-000000000001";
        let upper = container(&root.to_uppercase());
        let blueprint = blueprint(&json!({
            "/viewport": {"ViewportBlueprint": {"root_container": root}},
            container(root): {"ContainerBlueprint": {"container_kind": "Tabs", "contents": [upper.clone()]}},
            upper.clone(): {"ContainerBlueprint": {"container_kind": "Tabs"}},
        }));
        assert_eq!(
            problems(&blueprint),
            [format!(
                "`ContainerBlueprint.contents` of {} refers to {upper}, which has no \
                 ContainerBlueprint. Write it as {}: the viewer only finds lowercase, hyphenated \
                 UUIDs.",
                container(root),
                container(root),
            )]
        );
    }

    #[test]
    fn a_table_preview_keeps_its_view() {
        let blueprint = blueprint(&json!({
            "/table/layouts/cards/fields/link": {"TableColumnPreview": {"views": view(VIEW)}},
            view(VIEW): {"ViewBlueprint": {"class_identifier": "3D"}},
        }));
        assert!(problems(&blueprint).is_empty());
    }
}
