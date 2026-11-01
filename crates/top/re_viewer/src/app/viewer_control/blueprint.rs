//! JSON blueprint writes for viewer control, including defaults for explicitly supplied layouts.

use std::sync::Arc;

use re_chunk::{Chunk, ChunkError, RowId};
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::blueprint::archetypes::ViewportBlueprint;
use re_viewport_blueprint::VIEWPORT_PATH;

/// Convert JSON into chunks for a full blueprint replacement or a partial edit.
///
/// Full replacements clear omitted fields and entities; partial edits preserve them.
/// For full recording writes with an explicit root container, automatic layout & view options
/// are disabled by default if they're not set in JSON.
/// Returns the chunks without applying them to the blueprint store.
pub fn chunks_from_json(
    blueprint: &re_entity_db::EntityDb,
    json: &serde_json::Value,
    mode: re_protos::viewer_control::v1alpha1::BlueprintUpdateMode,
    is_recording_blueprint: bool,
    reflection: &re_sdk_types::reflection::Reflection,
) -> Result<Vec<Chunk>, re_log_channel::ViewerControlError> {
    use re_log_channel::ViewerControlError;
    use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

    let timepoint = re_viewer_context::blueprint_timepoint_for_writes(blueprint);
    let chunks = if mode == BlueprintUpdateMode::Partial {
        re_arrow_json::partial_chunks_from_json(json, &timepoint, reflection)
    } else {
        re_arrow_json::chunks_from_json(
            json,
            blueprint.storage_engine().store(),
            &timepoint,
            reflection,
        )
    };
    let mut chunks = chunks.map_err(|err| ViewerControlError::invalid_argument(err.to_string()))?;
    if mode != BlueprintUpdateMode::Partial && is_recording_blueprint {
        default_to_manual_layout(&mut chunks, &timepoint)
            .map_err(|err| ViewerControlError::invalid_argument(err.to_string()))?;
    }
    Ok(chunks)
}

/// The entity paths that `chunks` write to but `blueprint` does not hold yet, sorted.
pub fn created_entities(blueprint: &re_entity_db::EntityDb, chunks: &[Chunk]) -> Vec<String> {
    let created: std::collections::BTreeSet<&EntityPath> = chunks
        .iter()
        .map(Chunk::entity_path)
        .filter(|entity_path| !blueprint.is_logged_entity(entity_path))
        .collect();
    created.into_iter().map(ToString::to_string).collect()
}

/// Rejects `chunks` if writing them to `blueprint` would break a reference.
///
/// Only problems the write introduces count, so a blueprint that is already broken can still
/// be edited. A rejection also lists the problems that were there before and remain.
///
/// `undo_state` is the blueprint's undo history: the blueprint is judged as the viewer shows it,
/// at the undo cursor, and the write is judged after dropping the redo history, as writing does.
pub fn check_references(
    blueprint: &re_entity_db::EntityDb,
    undo_state: Option<&re_viewer_context::BlueprintUndoState>,
    chunks: &[Chunk],
) -> Result<(), re_log_channel::ViewerControlError> {
    use re_log_channel::ViewerControlError;
    use re_viewer_context::BlueprintUndoState;

    re_tracing::profile_function!();

    let mut written = re_entity_db::EntityDb::new(blueprint.store_id().clone());
    for chunk in blueprint.storage_engine().store().iter_physical_chunks() {
        written
            .add_chunk(chunk)
            .map_err(|err| ViewerControlError::internal(err.to_string()))?;
    }
    if let Some(undo_state) = undo_state {
        undo_state.clone().clear_redo_buffer(&mut written);
    }
    for chunk in chunks {
        written
            .add_chunk(&Arc::new(chunk.clone()))
            .map_err(|err| ViewerControlError::internal(err.to_string()))?;
    }

    let shown = undo_state.map_or_else(
        BlueprintUndoState::default_query,
        BlueprintUndoState::blueprint_query,
    );
    let before: std::collections::BTreeSet<_> =
        crate::blueprint::reference_problems(blueprint, &shown)
            .into_iter()
            .collect();
    let (remaining, introduced): (Vec<_>, Vec<_>) =
        crate::blueprint::reference_problems(&written, &BlueprintUndoState::default_query())
            .into_iter()
            .partition(|problem| before.contains(problem));
    if introduced.is_empty() {
        return Ok(());
    }

    let list = |problems: Vec<crate::blueprint::ReferenceProblem>| {
        problems
            .iter()
            .map(|problem| format!("- {problem}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let already_broken = if remaining.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nThe blueprint already had these problems before this write. \
             They do not block it, but you may want to fix them too:\n{}",
            list(remaining)
        )
    };
    let message = format!(
        "Nothing was written, since the blueprint would have broken references. \
         Fix all of these and send the whole edit again as one call:\n{}{already_broken}",
        list(introduced)
    );
    Err(ViewerControlError::invalid_argument(message))
}

/// When a root container is supplied, default omitted `auto_layout` and `auto_views` to false.
///
/// Preserve explicit options so callers can still request automatic layout or view creation.
fn default_to_manual_layout(
    chunks: &mut Vec<Chunk>,
    timepoint: &TimePoint,
) -> Result<(), ChunkError> {
    let viewport = EntityPath::from(VIEWPORT_PATH);
    let has_value = |component| {
        chunks
            .iter()
            .any(|chunk| chunk.entity_path() == &viewport && chunk.has_component_set(component))
    };
    if !has_value(ViewportBlueprint::descriptor_root_container().component) {
        return Ok(());
    }

    let needs_auto_layout = !has_value(ViewportBlueprint::descriptor_auto_layout().component);
    let needs_auto_views = !has_value(ViewportBlueprint::descriptor_auto_views().component);
    if needs_auto_layout || needs_auto_views {
        let mut options = ViewportBlueprint::update_fields();
        if needs_auto_layout {
            options = options.with_auto_layout(false);
        }
        if needs_auto_views {
            options = options.with_auto_views(false);
        }
        chunks.push(
            Chunk::builder(viewport)
                .with_archetype(RowId::new(), timepoint.clone(), &options)
                .build()?,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::LatestAtQuery;
    use re_log_types::{StoreId, StoreKind, TimelineName};
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_only_writes_that_break_references() {
        use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

        let root = "/container/aaaaaaaa-0000-4000-8000-000000000001";
        let shown = "/view/aaaaaaaa-0000-4000-8000-000000000002";
        let added = "/view/aaaaaaaa-0000-4000-8000-000000000003";
        let orphan = "/view/aaaaaaaa-0000-4000-8000-000000000004";
        let reflection = re_sdk_types::reflection::reflection();
        let mut db = re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        let apply = |db: &mut re_entity_db::EntityDb, json: serde_json::Value| {
            let chunks =
                chunks_from_json(db, &json, BlueprintUpdateMode::Partial, true, reflection)
                    .unwrap();
            check_references(db, None, &chunks)?;
            for chunk in chunks {
                db.add_chunk(&Arc::new(chunk)).unwrap();
            }
            Ok::<_, re_log_channel::ViewerControlError>(())
        };

        // An orphan from before does not block an unrelated edit.
        for chunk in chunks_from_json(
            &db,
            &json!({
                "/viewport": {"ViewportBlueprint": {"root_container": root.strip_prefix("/container/")}},
                root: {"ContainerBlueprint": {"container_kind": "Tabs", "contents": [shown]}},
                shown: {"ViewBlueprint": {"class_identifier": "3D"}},
                orphan: {"ViewBlueprint": {"class_identifier": "3D"}},
            }),
            BlueprintUpdateMode::Full,
            true,
            reflection,
        )
        .unwrap()
        {
            db.add_chunk(&Arc::new(chunk)).unwrap();
        }
        apply(
            &mut db,
            json!({shown: {"ViewBlueprint": {"display_name": "Kept"}}}),
        )
        .unwrap();

        // A view and the reference to it can only be added together.
        let new_view = json!({added: {"ViewBlueprint": {"class_identifier": "2D"}}});
        let err = apply(&mut db, new_view.clone()).unwrap_err();
        let (introduced, remaining) = err
            .message
            .split_once("already had")
            .expect("the old orphan is listed apart from the new problem");
        assert!(introduced.contains(added), "{}", err.message);
        assert!(remaining.contains(orphan), "{}", err.message);
        assert!(!remaining.contains(added), "{}", err.message);
        let new_reference = json!({root: {"ContainerBlueprint": {"contents": [shown, added]}}});
        let err = apply(&mut db, new_reference.clone()).unwrap_err();
        assert!(err.message.contains(added), "{}", err.message);
        let mut both = new_view;
        both.as_object_mut()
            .unwrap()
            .extend(new_reference.as_object().unwrap().clone());
        apply(&mut db, both).unwrap();
    }

    #[test]
    fn a_view_in_redo_history_does_not_count() {
        use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

        let root = "/container/aaaaaaaa-0000-4000-8000-000000000001";
        let kept = "/view/aaaaaaaa-0000-4000-8000-000000000002";
        let undone = "/view/aaaaaaaa-0000-4000-8000-000000000003";
        let reflection = re_sdk_types::reflection::reflection();
        let mut db = re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        for json in [
            json!({
                "/viewport": {"ViewportBlueprint": {"root_container": root.strip_prefix("/container/")}},
                root: {"ContainerBlueprint": {"container_kind": "Tabs", "contents": [kept]}},
                kept: {"ViewBlueprint": {"class_identifier": "3D"}},
            }),
            json!({
                root: {"ContainerBlueprint": {"contents": [kept, undone]}},
                undone: {"ViewBlueprint": {"class_identifier": "3D"}},
            }),
        ] {
            let chunks =
                chunks_from_json(&db, &json, BlueprintUpdateMode::Partial, true, reflection)
                    .unwrap();
            for chunk in chunks {
                db.add_chunk(&Arc::new(chunk)).unwrap();
            }
        }

        // Undo the second write, which created `undone`.
        let mut undo_state = re_viewer_context::BlueprintUndoState::default();
        undo_state.set_redo_time(re_log_types::TimeInt::new_temporal(1));

        let edit = json!({root: {"ContainerBlueprint": {"contents": [kept, undone]}}});
        let chunks =
            chunks_from_json(&db, &edit, BlueprintUpdateMode::Partial, true, reflection).unwrap();
        assert!(check_references(&db, None, &chunks).is_ok());
        let err = check_references(&db, Some(&undo_state), &chunks).unwrap_err();
        assert!(err.message.contains(undone), "{}", err.message);
    }

    #[test]
    fn a_view_deleted_in_the_ui_round_trips() {
        use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

        let root = "/container/aaaaaaaa-0000-4000-8000-000000000001";
        let kept = "/view/aaaaaaaa-0000-4000-8000-000000000002";
        let deleted = "/view/aaaaaaaa-0000-4000-8000-000000000003";
        let reflection = re_sdk_types::reflection::reflection();
        let mut db = re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        let write = |db: &mut re_entity_db::EntityDb, json: serde_json::Value| {
            let chunks =
                chunks_from_json(db, &json, BlueprintUpdateMode::Full, true, reflection).unwrap();
            check_references(db, None, &chunks).map_err(|err| err.message)?;
            for chunk in chunks {
                db.add_chunk(&Arc::new(chunk)).unwrap();
            }
            Ok::<_, String>(())
        };
        write(
            &mut db,
            json!({
                "/viewport": {"ViewportBlueprint": {"root_container": root.strip_prefix("/container/")}},
                root: {"ContainerBlueprint": {"container_kind": "Tabs", "contents": [kept, deleted]}},
                kept: {"ViewBlueprint": {"class_identifier": "3D"}},
                deleted: {"ViewBlueprint": {"class_identifier": "3D"}},
            }),
        )
        .unwrap();

        // What the viewer writes when the user removes a view.
        let timepoint = re_viewer_context::blueprint_timepoint_for_writes(&db);
        for chunk in re_arrow_json::partial_chunks_from_json(
            &json!({
                root: {"ContainerBlueprint": {"contents": [kept]}},
                deleted: {"Clear": {"is_recursive": true}},
            }),
            &timepoint,
            reflection,
        )
        .unwrap()
        {
            db.add_chunk(&Arc::new(chunk)).unwrap();
        }

        let read = re_arrow_json::json_from_store(
            db.storage_engine().store(),
            &LatestAtQuery::latest(re_viewer_context::blueprint_timeline()),
            reflection,
        );
        write(&mut db, read).unwrap();
    }

    #[test]
    fn reports_only_entities_the_write_creates() {
        use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

        let existing = "/view/aaaaaaaa-0000-4000-8000-000000000001";
        let typo = "/view/aaaaaaaa-0000-4000-8000-00000000000f";
        let reflection = re_sdk_types::reflection::reflection();
        let mut db = re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));

        let initial = json!({existing: {"ViewBlueprint": {"class_identifier": "3D"}}});
        let chunks =
            chunks_from_json(&db, &initial, BlueprintUpdateMode::Full, false, reflection).unwrap();
        assert_eq!(created_entities(&db, &chunks), [existing]);
        for chunk in chunks {
            db.add_chunk(&Arc::new(chunk)).unwrap();
        }

        let edit = json!({
            existing: {"ViewBlueprint": {"display_name": "Kept"}},
            typo: {"ViewBlueprint": {"display_name": "Ghost"}},
        });
        let chunks =
            chunks_from_json(&db, &edit, BlueprintUpdateMode::Partial, false, reflection).unwrap();
        assert_eq!(created_entities(&db, &chunks), [typo]);
    }

    /// Checks that partial writes preserve existing layout options, while full writes reset omissions.
    #[test]
    fn partial_mode_preserves_layout_options_and_unmentioned_entities() {
        use re_protos::viewer_control::v1alpha1::BlueprintUpdateMode;

        let root = "aaaaaaaa-0000-4000-8000-000000000001";
        for is_recording in [true, false] {
            for mode in [
                BlueprintUpdateMode::Full,
                BlueprintUpdateMode::Unspecified,
                BlueprintUpdateMode::Partial,
            ] {
                // Seed automatic options and a view that the subsequent write will omit.
                let mut db =
                    re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
                let initial = json!({
                    "/viewport": {"ViewportBlueprint": {"root_container": root, "auto_layout": true, "auto_views": true}},
                    "/view/aaaaaaaa-0000-4000-8000-000000000002": {"ViewBlueprint": {"class_identifier": "3D", "display_name": "Keep me"}}
                });
                let reflection = re_sdk_types::reflection::reflection();
                // Establish the full blueprint, then write only the root using each mode.
                for (json, mode) in [
                    (initial.clone(), BlueprintUpdateMode::Full),
                    (
                        json!({"/viewport": {"ViewportBlueprint": {"root_container": root}}}),
                        mode,
                    ),
                ] {
                    let chunks =
                        chunks_from_json(&db, &json, mode, is_recording, reflection).unwrap();
                    for chunk in chunks {
                        db.add_chunk(&Arc::new(chunk)).unwrap();
                    }
                }
                // Read the stored state to distinguish preservation from cleared omissions.
                let actual = re_arrow_json::json_from_store(
                    db.storage_engine().store(),
                    &LatestAtQuery::latest(re_viewer_context::blueprint_timeline()),
                    reflection,
                );
                if mode == BlueprintUpdateMode::Partial {
                    assert_eq!(actual, initial);
                } else {
                    assert!(
                        actual
                            .get("/view/aaaaaaaa-0000-4000-8000-000000000002")
                            .is_none()
                    );
                    if is_recording {
                        // Full recording writes supply manual defaults for an explicit root.
                        assert_eq!(
                            actual["/viewport"]["ViewportBlueprint"]["auto_layout"],
                            false
                        );
                        assert_eq!(
                            actual["/viewport"]["ViewportBlueprint"]["auto_views"],
                            false
                        );
                    } else {
                        // Table blueprints do not receive recording layout defaults.
                        assert!(
                            actual["/viewport"]["ViewportBlueprint"]
                                .get("auto_layout")
                                .is_none()
                        );
                    }
                }
            }
        }
    }

    /// Checks that an authored layout stays manual unless automatic behavior is explicitly requested.
    #[test]
    fn explicit_layout_defaults_preserve_opt_ins() {
        for (options, expected) in [
            (
                json!({}),
                json!({"auto_layout": false, "auto_views": false}),
            ),
            (
                json!({"auto_layout": true}),
                json!({"auto_layout": true, "auto_views": false}),
            ),
            (
                json!({"auto_views": true}),
                json!({"auto_layout": false, "auto_views": true}),
            ),
            (
                json!({"auto_layout": true, "auto_views": true}),
                json!({"auto_layout": true, "auto_views": true}),
            ),
        ] {
            // Existing automatic settings must not leak into the supplied layout.
            let mut db = re_entity_db::EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
            db.add_chunk(&Arc::new(
                Chunk::builder(VIEWPORT_PATH)
                    .with_archetype(
                        RowId::new(),
                        TimePoint::default(),
                        &ViewportBlueprint::new()
                            .with_auto_layout(true)
                            .with_auto_views(true),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();

            // Supplying a root opts into manual defaults; explicit options take precedence.
            let mut viewport = options;
            viewport["root_container"] = json!("aaaaaaaa-0000-4000-8000-000000000001");
            let input = json!({"/viewport": {"ViewportBlueprint": viewport}});
            let timepoint = TimePoint::default();
            let reflection = re_sdk_types::reflection::reflection();
            let mut chunks = re_arrow_json::chunks_from_json(
                &input,
                db.storage_engine().store(),
                &timepoint,
                reflection,
            )
            .unwrap();
            default_to_manual_layout(&mut chunks, &timepoint).unwrap();
            for chunk in chunks {
                db.add_chunk(&Arc::new(chunk)).unwrap();
            }
            // Query the stored result to account for cleared fields and row ordering.
            let output = re_arrow_json::json_from_store(
                db.storage_engine().store(),
                &LatestAtQuery::latest(TimelineName::from_static_str("blueprint")),
                reflection,
            );
            let actual = &output["/viewport"]["ViewportBlueprint"];
            assert_eq!(actual["root_container"], viewport["root_container"]);
            assert_eq!(actual["auto_layout"], expected["auto_layout"]);
            assert_eq!(actual["auto_views"], expected["auto_views"]);
        }
    }

    /// Checks that an empty blueprint can still use the viewer's automatic layout.
    #[test]
    fn empty_blueprint_keeps_automatic_defaults() {
        let mut chunks = Vec::new();
        default_to_manual_layout(&mut chunks, &TimePoint::default()).unwrap();
        assert!(chunks.is_empty());
    }
}
