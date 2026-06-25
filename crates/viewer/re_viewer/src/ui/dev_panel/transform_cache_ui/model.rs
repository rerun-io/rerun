use ahash::{HashMap, HashSet};
use re_chunk_store::{LatestAtQuery, MissingChunkReporter};
use re_sdk_types::TransformFrameIdHash;
use re_viewer_context::TransformDatabaseStoreCache;
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::external::re_tf::transform_cache_snapshot;

use super::FrameVisibilityFilter;
use re_viewer_context::external::re_tf::transform_cache_snapshot::Frame;

/// Thin UI wrapper around a filtered transform-cache snapshot.
///
/// TODO(michael): this currently only contains the lookups needed for the UI header summary,
/// extend when adding tree painting to the UI.
#[derive(Debug, Clone)]
pub(super) struct Model {
    /// Visible transform-cache snapshot.
    pub(super) snapshot: transform_cache_snapshot::Snapshot,

    /// Edge indices grouped by child frame for root detection.
    edge_indices_by_child: HashMap<TransformFrameIdHash, Vec<usize>>,

    pub(super) any_missing_chunks: bool,
}

/// Returns whether a frame is derived from an entity path.
fn is_implicit_frame(frame: &Frame) -> bool {
    frame.kind == transform_cache_snapshot::FrameKind::EntityPath
}

/// Filters used while building a transform-cache display model.
#[derive(Debug, Clone, Copy)]
pub(super) struct ModelFilter {
    pub(super) frame_filter: FrameVisibilityFilter,
    pub(super) edge_filter: transform_cache_snapshot::EdgeFilter,
}

impl ModelFilter {
    fn snapshot_filter(self) -> transform_cache_snapshot::SnapshotFilter {
        transform_cache_snapshot::SnapshotFilter {
            frames: match self.frame_filter {
                FrameVisibilityFilter::All => transform_cache_snapshot::FrameFilter::All,
                FrameVisibilityFilter::Implicit => {
                    transform_cache_snapshot::FrameFilter::EntityPath
                }
                FrameVisibilityFilter::Named | FrameVisibilityFilter::Unlinked => {
                    transform_cache_snapshot::FrameFilter::Named
                }
            },
            edges: self.edge_filter,
        }
    }

    fn shows_node(self, node: &Frame) -> bool {
        let is_implicit = is_implicit_frame(node);
        match self.frame_filter {
            FrameVisibilityFilter::All => true,
            FrameVisibilityFilter::Implicit => is_implicit,
            FrameVisibilityFilter::Named => !is_implicit && node.has_transform,
            FrameVisibilityFilter::Unlinked => !is_implicit && !node.has_transform,
        }
    }
}

/// Builds a display model from the transform cache at one latest-at time.
pub(super) fn build_transform_cache_model(
    recording: &EntityDb,
    cache: &mut TransformDatabaseStoreCache,
    query: &LatestAtQuery,
    filter: ModelFilter,
) -> Model {
    let missing_chunk_reporter = MissingChunkReporter::default();
    let mut snapshot = cache.latest_at_transform_cache_snapshot(
        recording,
        &missing_chunk_reporter,
        query,
        filter.snapshot_filter(),
    );

    let connected_frames = snapshot
        .edges
        .iter()
        .flat_map(|edge| [edge.parent, edge.child])
        .collect::<HashSet<_>>();

    snapshot.frames.retain(|frame| {
        (connected_frames.contains(&frame.id)
            || (!is_implicit_frame(frame) && !frame.has_transform))
            && filter.shows_node(frame)
    });
    snapshot
        .frames
        .sort_by(|a, b| a.label.as_str().cmp(b.label.as_str()));

    let visible_frames = snapshot
        .frames
        .iter()
        .map(|frame| frame.id)
        .collect::<HashSet<_>>();
    snapshot.edges.retain(|edge| {
        visible_frames.contains(&edge.parent) && visible_frames.contains(&edge.child)
    });
    let frame_labels = snapshot
        .frames
        .iter()
        .map(|frame| (frame.id, frame.label.to_string()))
        .collect::<HashMap<_, _>>();
    snapshot.edges.sort_by_key(|edge| {
        (
            frame_labels
                .get(&edge.parent)
                .cloned()
                .unwrap_or_else(|| format!("{:?}", edge.parent)),
            frame_labels
                .get(&edge.child)
                .cloned()
                .unwrap_or_else(|| format!("{:?}", edge.child)),
        )
    });

    Model::new(snapshot, missing_chunk_reporter.any_missing())
}

impl Model {
    fn new(snapshot: transform_cache_snapshot::Snapshot, any_missing_chunks: bool) -> Self {
        let mut edge_indices_by_child: HashMap<TransformFrameIdHash, Vec<usize>> =
            Default::default();

        for (edge_index, edge) in snapshot.edges.iter().enumerate() {
            edge_indices_by_child
                .entry(edge.child)
                .or_default()
                .push(edge_index);
        }

        Self {
            snapshot,
            edge_indices_by_child,
            any_missing_chunks,
        }
    }

    /// Counts visible root components in the graph.
    pub(super) fn num_trees(&self) -> usize {
        self.snapshot
            .frames
            .iter()
            .filter(|node| !self.edge_indices_by_child.contains_key(&node.id))
            .count()
    }
}
