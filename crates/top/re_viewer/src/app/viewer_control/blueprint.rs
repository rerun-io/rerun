//! JSON blueprint writes for viewer control, including defaults for explicitly supplied layouts.

use re_chunk::{Chunk, ChunkError, RowId};
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::blueprint::archetypes::ViewportBlueprint;
use re_viewport_blueprint::VIEWPORT_PATH;

/// Convert JSON into chunks that replace the current blueprint, clearing omitted fields and entities.
///
/// For recordings with an explicit root container, automatic layout & view options are disabled
/// by default if they're not set in JSON.
/// Returns the chunks without applying them to the blueprint store.
pub fn chunks_from_json(
    blueprint: &re_entity_db::EntityDb,
    json: &serde_json::Value,
    is_recording_blueprint: bool,
    reflection: &re_sdk_types::reflection::Reflection,
) -> Result<Vec<Chunk>, re_log_channel::ViewerControlError> {
    use re_log_channel::ViewerControlError;

    let timepoint = re_viewer_context::blueprint_timepoint_for_writes(blueprint);
    let chunks = re_arrow_json::chunks_from_json(
        json,
        blueprint.storage_engine().store(),
        &timepoint,
        reflection,
    );
    let mut chunks = chunks.map_err(|err| ViewerControlError::invalid_argument(err.to_string()))?;
    if is_recording_blueprint {
        default_to_manual_layout(&mut chunks, &timepoint)
            .map_err(|err| ViewerControlError::invalid_argument(err.to_string()))?;
    }
    Ok(chunks)
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
