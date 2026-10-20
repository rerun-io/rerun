use std::collections::HashMap;

use re_chunk_store::MissingChunkReporter;
use re_log_channel::ViewerControlError;
use re_log_types::StoreId;
use re_protos::viewer_control::v1alpha1::{
    GetTransformGraphRequest, GetTransformGraphResponse, get_transform_graph_request,
};
use re_viewer_context::external::re_tf::transform_cache_snapshot::{
    EdgeFilter, EdgeSource, FrameFilter, FrameKind, Snapshot, SnapshotFilter, SubspaceKind,
};
use re_viewer_context::{StoreHub, TransformDatabaseStoreCache};
use serde_json::{Value, json};

use super::App;

impl App {
    /// Returns a filtered transform graph snapshot at the recording's current timeline and time cursor.
    ///
    /// If no time cursor is set, queries the latest values on the selected timeline.
    /// The response includes the query time and options, and indicates whether any chunks were missing.
    pub fn collect_transform_graph(
        &self,
        store_hub: &StoreHub,
        request: GetTransformGraphRequest,
    ) -> Result<GetTransformGraphResponse, ViewerControlError> {
        let frame_filter = match request.frame_filter {
            None => FrameFilter::All,
            Some(value) => match get_transform_graph_request::FrameFilter::try_from(value) {
                Ok(
                    get_transform_graph_request::FrameFilter::Unspecified
                    | get_transform_graph_request::FrameFilter::All,
                ) => FrameFilter::All,
                Ok(get_transform_graph_request::FrameFilter::EntityPath) => FrameFilter::EntityPath,
                Ok(get_transform_graph_request::FrameFilter::Named) => FrameFilter::Named,
                Err(err) => {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "invalid frame_filter: {err}"
                    )));
                }
            },
        };
        let edge_filter = match request.edge_filter {
            None => EdgeFilter::All,
            Some(value) => match get_transform_graph_request::EdgeFilter::try_from(value) {
                Ok(
                    get_transform_graph_request::EdgeFilter::Unspecified
                    | get_transform_graph_request::EdgeFilter::All,
                ) => EdgeFilter::All,
                Ok(get_transform_graph_request::EdgeFilter::Static) => EdgeFilter::Static,
                Ok(get_transform_graph_request::EdgeFilter::Temporal) => EdgeFilter::Temporal,
                Err(err) => {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "invalid edge_filter: {err}"
                    )));
                }
            },
        };
        let include_transforms = request.include_transforms.unwrap_or(false);
        let store_id = request
            .store_id
            .map(|id| id.parse::<StoreId>())
            .transpose()
            .map_err(|err| {
                ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
            })?
            .or_else(|| self.state.active_recording_id().cloned())
            .ok_or_else(|| {
                ViewerControlError::failed_precondition("no active recording, and no `store_id`")
            })?;
        let recording = store_hub.entity_db(&store_id).ok_or_else(|| {
            ViewerControlError::not_found(format!("recording {store_id} is not open"))
        })?;
        let time_control = self.state.time_control(&store_id).ok_or_else(|| {
            ViewerControlError::failed_precondition(
                "no time control for this recording; select it in the viewer first",
            )
        })?;
        let query = time_control.current_query();
        let caches = store_hub.store_caches(&store_id).ok_or_else(|| {
            ViewerControlError::failed_precondition("no transform cache for this recording yet")
        })?;
        let missing = MissingChunkReporter::default();
        let snapshot = caches.memoizer(|cache: &mut TransformDatabaseStoreCache| {
            cache.latest_at_transform_cache_snapshot(
                recording,
                &missing,
                &query,
                SnapshotFilter {
                    frames: frame_filter,
                    edges: edge_filter,
                },
            )
        });
        let mut graph = snapshot_json(&snapshot, include_transforms);
        graph["query"] = json!({
            "timeline": time_control.timeline_name().to_string(),
            "time_type": time_control.time_type().map(|kind| match kind {
                re_log_types::TimeType::Sequence => "sequence",
                re_log_types::TimeType::DurationNs => "duration_ns",
                re_log_types::TimeType::TimestampNs => "timestamp_ns",
            }),
            "mode": if time_control.time_int().is_some() { "at_cursor" } else { "latest" },
            "time": time_control.time_int().map(|time| time.as_i64().to_string()),
        });
        graph["options"] = json!({
            "frame_filter": match frame_filter {
                FrameFilter::All => "all", FrameFilter::EntityPath => "entity_path", FrameFilter::Named => "named",
            },
            "edge_filter": match edge_filter {
                EdgeFilter::All => "all", EdgeFilter::Static => "static", EdgeFilter::Temporal => "temporal",
            },
            "include_transforms": include_transforms,
        });
        graph["missing_chunks"] = json!(missing.any_missing());
        Ok(GetTransformGraphResponse {
            store_id: store_id.to_string(),
            json: graph.to_string(),
        })
    }
}

/// Encodes a transform graph snapshot as JSON with frames and edges sorted by frame names.
///
/// Numerical transform values can be left out by setting `include_transforms` to false.
fn snapshot_json(snapshot: &Snapshot, include_transforms: bool) -> Value {
    let labels: HashMap<_, _> = snapshot
        .frames
        .iter()
        .map(|frame| (frame.id, frame.label.as_str()))
        .collect();
    let mut frames = snapshot.frames.iter().collect::<Vec<_>>();
    frames.sort_by_key(|frame| frame.label.as_str());
    let frames = frames.into_iter().map(|frame| json!({
        "id": frame.label.as_str(),
        "kind": match frame.kind { FrameKind::EntityPath => "entity_path", FrameKind::Named => "named" },
        "subspace": match frame.subspace_kind { SubspaceKind::TwoD => "2D", SubspaceKind::ThreeD => "3D" },
        "has_transform": frame.has_transform,
    })).collect::<Vec<_>>();
    let mut edges = snapshot.edges.iter().collect::<Vec<_>>();
    edges.sort_by_key(|edge| {
        (
            labels[&edge.parent],
            labels[&edge.child],
            match edge.source {
                EdgeSource::ImplicitHierarchy => 0,
                EdgeSource::Transform { .. } => 1,
                EdgeSource::Pinhole { .. } => 2,
            },
        )
    });
    let edges = edges
        .into_iter()
        .map(|edge| {
            let source = match &edge.source {
                EdgeSource::ImplicitHierarchy => json!({ "kind": "implicit_hierarchy" }),
                EdgeSource::Transform {
                    entity_path,
                    transform,
                } => {
                    let mut source =
                        json!({ "kind": "transform", "entity_path": entity_path.to_string() });
                    if include_transforms {
                        source["parent_from_child"] = json!(
                            glam::DMat4::from(transform.transform)
                                .transpose()
                                .to_cols_array_2d()
                        );
                    }
                    source
                }
                EdgeSource::Pinhole {
                    entity_path,
                    pinhole,
                } => {
                    let mut source =
                        json!({ "kind": "pinhole", "entity_path": entity_path.to_string() });
                    if include_transforms {
                        source["image_from_camera"] = json!(
                            glam::Mat3::from_cols_array(&pinhole.image_from_camera.0.0)
                                .transpose()
                                .to_cols_array_2d()
                        );
                        source["resolution"] =
                            json!(pinhole.resolution.map(|resolution| resolution.0.0));
                        source["camera_xyz"] = json!(pinhole.view_coordinates.describe_short());
                    }
                    source
                }
            };
            json!({
                "parent": labels[&edge.parent], "child": labels[&edge.child],
                "sample_time": if edge.time.is_static() { json!({ "kind": "static" }) }
                    else { json!({ "kind": "temporal", "value": edge.time.as_i64().to_string() }) },
                "source": source,
            })
        })
        .collect::<Vec<_>>();
    json!({ "frames": frames, "edges": edges })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, LatestAtQuery, Timeline};
    use re_entity_db::EntityDb;
    use re_log_types::StoreKind;
    use re_sdk_types::archetypes::{CoordinateFrame, Pinhole, Transform3D};

    use super::*;

    /// Checks matrix direction and row layout, optional values, isolated frames, and stable JSON ordering.
    #[test]
    fn snapshot_json_contract() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = EntityDb::new(StoreId::random(
            StoreKind::Recording,
            "transform_graph_test",
        ));
        let timeline = Timeline::new_sequence("frame");
        db.add_chunk(&Arc::new(
            Chunk::builder("tf")
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::from_translation([2.0, 3.0, 4.0])
                        .with_parent_frame("world")
                        .with_child_frame("camera"),
                )
                .build()?,
        ))?;
        db.add_chunk(&Arc::new(
            Chunk::builder("intrinsics")
                .with_archetype_auto_row(
                    re_log_types::TimePoint::default(),
                    &Pinhole::from_focal_length_and_resolution([100.0, 200.0], [640.0, 480.0])
                        .with_parent_frame("camera")
                        .with_child_frame("image"),
                )
                .build()?,
        ))?;
        db.add_chunk(&Arc::new(
            Chunk::builder("unlinked")
                .with_archetype_auto_row(
                    re_log_types::TimePoint::default(),
                    &CoordinateFrame::new("isolated"),
                )
                .build()?,
        ))?;
        let mut cache = TransformDatabaseStoreCache::default();
        let missing = MissingChunkReporter::default();
        let first = cache.latest_at_transform_cache_snapshot(
            &db,
            &missing,
            &LatestAtQuery::new(*timeline.name(), 1),
            SnapshotFilter::default(),
        );
        let compact = snapshot_json(&first, false);
        let full = snapshot_json(&first, true);
        let edge = |graph: &Value, kind: &str| {
            graph["edges"]
                .as_array()
                .expect("graph must contain an edges array")
                .iter()
                .find(|edge| edge["source"]["kind"] == kind)
                .expect("fixture must contain an edge of the requested kind")
                .clone()
        };
        let transform = edge(&full, "transform");
        assert_eq!(transform["parent"], "world");
        assert_eq!(transform["child"], "camera");
        assert_eq!(
            transform["sample_time"],
            json!({ "kind": "temporal", "value": "1" })
        );
        assert_eq!(
            transform["source"]["parent_from_child"],
            json!([
                [1.0, 0.0, 0.0, 2.0],
                [0.0, 1.0, 0.0, 3.0],
                [0.0, 0.0, 1.0, 4.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        );
        assert!(
            edge(&compact, "transform")["source"]
                .get("parent_from_child")
                .is_none()
        );
        let pinhole = edge(&full, "pinhole");
        assert_eq!(pinhole["sample_time"], json!({ "kind": "static" }));
        assert_eq!(
            pinhole["source"]["image_from_camera"],
            json!([[100.0, 0.0, 320.0], [0.0, 200.0, 240.0], [0.0, 0.0, 1.0],])
        );
        assert_eq!(pinhole["source"]["resolution"], json!([640.0, 480.0]));
        assert_eq!(pinhole["source"]["camera_xyz"], "RDF");
        assert!(
            edge(&compact, "pinhole")["source"]
                .get("image_from_camera")
                .is_none()
        );
        assert!(
            compact["frames"]
                .as_array()
                .expect("graph must contain a frames array")
                .iter()
                .any(|frame| frame["id"] == "isolated" && frame["has_transform"] == false)
        );
        let mut reordered = first.clone();
        reordered.frames.reverse();
        reordered.edges.reverse();
        assert_eq!(snapshot_json(&reordered, true), full);
        assert!(!missing.any_missing());
        Ok(())
    }
}
