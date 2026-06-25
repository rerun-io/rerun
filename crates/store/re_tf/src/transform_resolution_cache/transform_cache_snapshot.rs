//! Snapshots for inspecting the transform cache at a single latest-at time.
//!
//! A snapshot describes the direct frame graph known to the cache: registered frames are graph
//! nodes, and latest direct transforms between frames are graph edges.

use ahash::HashSet;
use re_chunk_store::{LatestAtQuery, MissingChunkReporter};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::components::TransformFrameId;

use crate::TransformFrameIdHash;
use crate::frame_id_registry::FrameIdRegistry;

use super::cached_transforms_for_timeline::CachedTransformsForTimeline;
use super::parent_from_child_transform::ParentFromChildTransform;
use super::resolved_pinhole_projection::ResolvedPinholeProjection;
use super::tree_transforms_for_child_frame::TreeTransformsForChildFrame;

/// Which transform-cache snapshot edges should be returned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeFilter {
    /// Include all static and temporal edges.
    #[default]
    All,

    /// Include only static edges.
    Static,

    /// Include only temporal edges.
    Temporal,
}

impl EdgeFilter {
    #[inline]
    pub fn includes(self, time: TimeInt) -> bool {
        match self {
            Self::All => true,
            Self::Static => time.is_static(),
            Self::Temporal => !time.is_static(),
        }
    }
}

/// Which transform-cache snapshot frames should be returned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameFilter {
    /// Include all registered frames.
    #[default]
    All,

    /// Include only frames derived from entity paths.
    EntityPath,

    /// Include only explicitly named frames.
    Named,
}

impl FrameFilter {
    #[inline]
    pub fn includes(self, kind: FrameKind) -> bool {
        match self {
            Self::All => true,
            Self::EntityPath => kind == FrameKind::EntityPath,
            Self::Named => kind == FrameKind::Named,
        }
    }
}

/// Filter for a transform-cache snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotFilter {
    pub frames: FrameFilter,
    pub edges: EdgeFilter,
}

/// The source category of a registered transform frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    EntityPath,
    Named,
}

/// Whether a transform frame belongs to a 2D or 3D subspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubspaceKind {
    TwoD,
    ThreeD,
}

/// Information about a registered transform frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub id: TransformFrameIdHash,
    pub label: TransformFrameId,
    pub kind: FrameKind,
    pub subspace_kind: SubspaceKind,

    /// Whether this frame participates in any latest-at transform.
    pub has_transform: bool,
}

/// Where does the parent-child transform edge originate from?
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeSource {
    ImplicitHierarchy,
    Transform {
        entity_path: EntityPath,
        transform: ParentFromChildTransform,
    },
    Pinhole {
        entity_path: EntityPath,
        pinhole: ResolvedPinholeProjection,
    },
}

/// A transform-cache snapshot edge between a child frame and its parent frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub parent: TransformFrameIdHash,
    pub child: TransformFrameIdHash,
    pub time: TimeInt,
    pub source: EdgeSource,
}

/// Snapshot of the transform cache at a single latest-at time.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub frames: Vec<Frame>,
    pub edges: Vec<Edge>,
}

/// Returns a snapshot of the transform cache for a single latest-at time.
///
/// The snapshot contains registered frames matching the frame filter plus latest direct transform
/// edges between them.
///
/// `filter` defines which frame and edge kinds shall be included in the result.
pub fn latest_at(
    transforms: &CachedTransformsForTimeline,
    frame_id_registry: &FrameIdRegistry,
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    query: &LatestAtQuery,
    filter: SnapshotFilter,
) -> Snapshot {
    // Collect all logged transform edges.
    let logged_edges =
        latest_at_logged_transform_edges(transforms, entity_db, missing_chunk_reporter, query);
    let children_with_logged_transforms = logged_edges
        .iter()
        .map(|edge| edge.child)
        .collect::<HashSet<_>>();

    // First, collect all frames and edges of logged transforms that are compatible with the edge
    // filter. Frame filtering happens in a second step later.
    let mut two_d_frames = HashSet::default();
    let mut frames_with_transforms = HashSet::default();
    let mut edges = Vec::new();
    for edge in logged_edges {
        frames_with_transforms.insert(edge.parent);
        frames_with_transforms.insert(edge.child);
        if matches!(edge.source, EdgeSource::Pinhole { .. }) {
            two_d_frames.insert(edge.child);
        }
        if filter.edges.includes(edge.time) {
            edges.push(edge);
        }
    }

    // Entity-path-derived frames are implicit relationships that don't necessarily have a logged
    // transform (identity transform as default).
    // Collect these entity-path-derived frames that we haven't yet seen as logged transforms.
    for (parent, child) in frame_id_registry.iter_entity_path_hierarchy_edges() {
        if children_with_logged_transforms.contains(&child) {
            continue;
        }

        frames_with_transforms.insert(parent);
        frames_with_transforms.insert(child);

        // Implicit identity transforms are static.
        if filter.edges.includes(TimeInt::STATIC) {
            edges.push(Edge {
                parent,
                child,
                time: TimeInt::STATIC,
                source: EdgeSource::ImplicitHierarchy,
            });
        }
    }

    // Retrieve the frame information for all frames that match the frame filter.
    let mut returned_frames = HashSet::default();
    let frames = frame_id_registry
        .iter_frame_ids()
        .filter_map(|(id, label)| {
            let kind = if label.as_entity_path().is_some() {
                FrameKind::EntityPath
            } else {
                FrameKind::Named
            };

            if !filter.frames.includes(kind) {
                return None;
            }

            returned_frames.insert(*id);
            Some(Frame {
                id: *id,
                label: label.clone(),
                kind,
                subspace_kind: if two_d_frames.contains(id) {
                    SubspaceKind::TwoD
                } else {
                    SubspaceKind::ThreeD
                },
                has_transform: frames_with_transforms.contains(id),
            })
        })
        .collect::<Vec<_>>();

    // Frame filtering can hide edge endpoints, so prune edges after collecting returned frames.
    edges.retain(|edge| {
        returned_frames.contains(&edge.parent) && returned_frames.contains(&edge.child)
    });

    Snapshot { frames, edges }
}

/// Returns the latest logged transform and pinhole edges for the requested time.
fn latest_at_logged_transform_edges(
    transforms: &CachedTransformsForTimeline,
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    query: &LatestAtQuery,
) -> Vec<Edge> {
    let mut child_transforms = transforms
        .per_child_frame_transforms
        .iter()
        .collect::<Vec<_>>();
    child_transforms.sort_unstable_by_key(|(child, _)| **child);

    child_transforms
        .into_iter()
        .flat_map(|(_, transforms)| {
            [
                latest_at_transform_edge(transforms, entity_db, missing_chunk_reporter, query),
                latest_at_pinhole_edge(transforms, entity_db, missing_chunk_reporter, query),
            ]
            .into_iter()
            .flatten()
        })
        .collect()
}

/// Returns the latest logged transform edge for the child frame.
fn latest_at_transform_edge(
    transforms: &TreeTransformsForChildFrame,
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    query: &LatestAtQuery,
) -> Option<Edge> {
    let (time, transform) =
        transforms.latest_at_transform_with_metadata(entity_db, missing_chunk_reporter, query)?;

    Some(Edge {
        parent: transform.parent,
        child: transforms.child_frame,
        time,
        source: EdgeSource::Transform {
            entity_path: transforms.associated_entity_path(time).clone(),
            transform,
        },
    })
}

/// Returns the latest logged pinhole edge for the child frame.
fn latest_at_pinhole_edge(
    transforms: &TreeTransformsForChildFrame,
    entity_db: &EntityDb,
    missing_chunk_reporter: &MissingChunkReporter,
    query: &LatestAtQuery,
) -> Option<Edge> {
    let (time, pinhole) =
        transforms.latest_at_pinhole_with_metadata(entity_db, missing_chunk_reporter, query)?;

    Some(Edge {
        parent: pinhole.parent,
        child: transforms.child_frame,
        time,
        source: EdgeSource::Pinhole {
            entity_path: transforms.associated_entity_path(time).clone(),
            pinhole,
        },
    })
}
