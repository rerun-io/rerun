use ahash::{HashMap, HashSet};
use re_chunk_store::{LatestAtQuery, MissingChunkReporter};
use re_sdk_types::TransformFrameIdHash;
use re_viewer_context::TransformDatabaseStoreCache;
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::external::re_tf::transform_cache_snapshot;

use super::FrameVisibilityFilter;
pub(super) use re_viewer_context::external::re_tf::transform_cache_snapshot::{
    Edge, Frame as Node, SubspaceKind,
};

/// Thin UI wrapper around a filtered transform-cache snapshot.
///
/// The wrapper adds only the derived graph lookups needed for layout, hover highlighting, and
/// tooltip labels.
#[derive(Debug, Clone)]
pub(super) struct Model {
    /// Visible transform-cache snapshot.
    pub(super) snapshot: transform_cache_snapshot::Snapshot,

    /// Edge indices grouped by parent frame for layout and shared-fork drawing.
    pub(super) edge_indices_by_parent: HashMap<TransformFrameIdHash, Vec<usize>>,

    /// Edge indices grouped by child frame for root detection and ancestor traversal.
    pub(super) edge_indices_by_child: HashMap<TransformFrameIdHash, Vec<usize>>,

    /// Node indices keyed by frame id for label lookups without duplicating labels.
    node_indices_by_id: HashMap<TransformFrameIdHash, usize>,

    pub(super) any_missing_chunks: bool,
}

/// Returns whether a frame is derived from an entity path.
pub(super) fn is_implicit_frame(frame: &Node) -> bool {
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

    fn shows_node(self, node: &Node) -> bool {
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
        let mut edge_indices_by_parent: HashMap<TransformFrameIdHash, Vec<usize>> =
            Default::default();
        let mut edge_indices_by_child: HashMap<TransformFrameIdHash, Vec<usize>> =
            Default::default();

        for (edge_index, edge) in snapshot.edges.iter().enumerate() {
            edge_indices_by_parent
                .entry(edge.parent)
                .or_default()
                .push(edge_index);
            edge_indices_by_child
                .entry(edge.child)
                .or_default()
                .push(edge_index);
        }

        let node_indices_by_id = snapshot
            .frames
            .iter()
            .enumerate()
            .map(|(node_index, node)| (node.id, node_index))
            .collect();

        Self {
            snapshot,
            edge_indices_by_parent,
            edge_indices_by_child,
            node_indices_by_id,
            any_missing_chunks,
        }
    }

    /// Returns the user-facing label for a frame id.
    pub(super) fn frame_label(&self, frame: TransformFrameIdHash) -> &str {
        self.node_indices_by_id
            .get(&frame)
            .and_then(|&node_index| self.snapshot.frames.get(node_index))
            .map_or("<unknown>", |node| node.label.as_str())
    }

    /// Returns a stable sort key for root ordering.
    pub(super) fn sort_key(&self, frame: TransformFrameIdHash) -> &str {
        self.node_indices_by_id
            .get(&frame)
            .and_then(|&node_index| self.snapshot.frames.get(node_index))
            .map_or("", |node| node.label.as_str())
    }

    /// Returns true when an edge leaves a parent through a shared fork path.
    pub(super) fn edge_starts_at_shared_fork(&self, edge: &Edge) -> bool {
        self.edge_indices_by_parent
            .get(&edge.parent)
            .is_some_and(|children| children.len() > 1)
    }

    /// Returns the number of visible child transforms for a frame.
    pub(super) fn num_children(&self, frame: TransformFrameIdHash) -> usize {
        self.edge_indices_by_parent.get(&frame).map_or(0, Vec::len)
    }

    /// Collects all visible ancestors of a frame and the edges on the path to them.
    pub(super) fn path_to_roots(
        &self,
        frame: TransformFrameIdHash,
    ) -> (HashSet<TransformFrameIdHash>, HashSet<usize>) {
        let mut ancestors = HashSet::default();
        let mut edge_indices = HashSet::default();
        self.collect_path_to_roots(frame, &mut ancestors, &mut edge_indices);
        (ancestors, edge_indices)
    }

    /// Counts visible root components in the graph.
    pub(super) fn num_trees(&self) -> usize {
        self.snapshot
            .frames
            .iter()
            .filter(|node| !self.edge_indices_by_child.contains_key(&node.id))
            .count()
    }

    /// Recursively collects root paths while protecting against cycles.
    fn collect_path_to_roots(
        &self,
        frame: TransformFrameIdHash,
        ancestors: &mut HashSet<TransformFrameIdHash>,
        edge_indices: &mut HashSet<usize>,
    ) {
        for &edge_index in self.edge_indices_by_child.get(&frame).into_iter().flatten() {
            let edge = &self.snapshot.edges[edge_index];
            edge_indices.insert(edge_index);
            if ancestors.insert(edge.parent) {
                self.collect_path_to_roots(edge.parent, ancestors, edge_indices);
            }
        }
    }
}
