use std::sync::Arc;

use ahash::HashMap;
use parking_lot::{ArcRwLockReadGuard, RawRwLock, RwLock};
use re_byte_size::SizeBytes;
use re_chunk_store::ChunkStore;
use re_entity_db::EntityDb;
use re_log::{debug_assert, debug_assert_eq};
use re_log_types::{TimeInt, TimelineName};
use re_sdk_types::archetypes;

use crate::frame_id_registry::FrameIdRegistry;
use crate::transform_aspect::TransformAspect;

use super::cached_transforms_for_timeline::CachedTransformsForTimeline;
use super::iter_child_frames_in_chunk;

type ArcRwLock<T> = Arc<RwLock<T>>;

/// Resolves all transform relationship defining components to affine transforms for fast lookup.
///
/// It only handles resulting transforms individually to each frame connection, not how these transforms propagate in the tree.
/// For transform tree propagation see [`crate::TransformForest`].
///
/// There are different kinds of transforms handled here:
/// * [`archetypes::Transform3D`]
///   Tree transforms that should propagate in the tree (via [`crate::TransformForest`]).
/// * [`re_sdk_types::components::PinholeProjection`] and [`re_sdk_types::components::ViewCoordinates`]
///   Pinhole projections & associated view coordinates used for visualizing cameras in 3D and embedding 2D in 3D
/// * [`archetypes::InstancePoses3D`]
///   Instance poses that should be applied to the tree transforms (via [`crate::TransformForest`]) but not propagate.
///   Also unlike tree transforms, these are not associated with transform frames but rather with entity paths.
pub struct TransformResolutionCache {
    /// The frame id registry is co-located in the resolution cache for convenience:
    /// the resolution cache is often the lowest level of transform access and
    /// thus allowing us to access debug information across the stack.
    frame_id_registry: ArcRwLock<FrameIdRegistry>,

    /// The timelines for which we have cached transforms for.
    ///
    /// Some timelines may be missing from this map.
    /// They will be lazily initialized from scratch on-demand.
    per_timeline: HashMap<TimelineName, ArcRwLock<CachedTransformsForTimeline>>,

    static_timeline: ArcRwLock<CachedTransformsForTimeline>,
}

impl Default for TransformResolutionCache {
    #[inline]
    fn default() -> Self {
        Self {
            frame_id_registry: Default::default(),
            per_timeline: Default::default(),
            static_timeline: Arc::new(RwLock::new(CachedTransformsForTimeline::new_static())),
        }
    }
}

impl TransformResolutionCache {
    /// Creates a new cache, initialized with the static timeline and frame registry from the given entity database.
    ///
    /// Per-timeline data will be lazily initialized on demand.
    pub fn new(entity_db: &EntityDb) -> Self {
        re_tracing::profile_function!();

        let mut cache = Self::default();

        for chunk in entity_db.storage_engine().store().iter_physical_chunks() {
            // Register all frames even if this chunk doesn't have transform data.
            cache
                .frame_id_registry
                .write()
                .register_all_frames_in_chunk(chunk);

            let aspects = TransformAspect::transform_aspects_of(chunk);
            if aspects.is_empty() {
                continue;
            }

            // Only process static chunks - temporal data will be lazily initialized per-timeline.
            if chunk.is_static() {
                cache.add_static_chunk(chunk, aspects);
            }
        }

        cache
    }
}

impl SizeBytes for TransformResolutionCache {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            frame_id_registry,
            per_timeline,
            static_timeline,
        } = self;

        frame_id_registry.heap_size_bytes()
            + per_timeline.heap_size_bytes()
            + static_timeline.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for TransformResolutionCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            frame_id_registry,
            per_timeline,
            static_timeline,
        } = self;

        let mut per_timeline_node = re_byte_size::MemUsageNode::new();
        for (timeline, cached_transforms) in per_timeline {
            per_timeline_node = per_timeline_node.with_child(
                timeline.to_string(),
                cached_transforms.read().capture_mem_usage_tree(),
            );
        }

        re_byte_size::MemUsageNode::new()
            .with_child("frame_id_registry", frame_id_registry.total_size_bytes())
            .with_child("per_timeline", per_timeline_node.into_tree())
            .with_child("static_timeline", static_timeline.total_size_bytes())
            .into_tree()
    }
}

impl TransformResolutionCache {
    /// Returns the registry of all known frame ids.
    #[inline]
    pub fn frame_id_registry(&self) -> ArcRwLockReadGuard<RawRwLock, FrameIdRegistry> {
        self.frame_id_registry.read_arc()
    }

    /// Accesses the transform component tracking data for a given timeline.
    #[inline]
    pub fn transforms_for_timeline(
        &self,
        timeline: TimelineName,
    ) -> ArcRwLockReadGuard<RawRwLock, CachedTransformsForTimeline> {
        if let Some(per_timeline) = self.per_timeline.get(&timeline) {
            per_timeline.read_arc()
        } else {
            self.static_timeline.read_arc()
        }
    }

    /// Returns an iterator over all initialized timelines.
    #[inline]
    pub fn cached_timelines(&self) -> impl Iterator<Item = TimelineName> + '_ {
        self.per_timeline.keys().copied()
    }

    /// Ensure we have a cache for this timeline.
    pub fn ensure_timeline_is_initialized(
        &mut self,
        chunk_store: &ChunkStore,
        timeline: TimelineName,
    ) {
        re_tracing::profile_function!(timeline);

        let static_timeline = self.static_timeline.read();
        let frame_id_registry = self.frame_id_registry.read();

        self.per_timeline.entry(timeline).or_insert_with(|| {
            Arc::new(RwLock::new(CachedTransformsForTimeline::new_temporal(
                timeline,
                &static_timeline,
                &frame_id_registry,
                chunk_store,
            )))
        });
    }

    /// Evicts a timeline from the cache.
    pub fn evict_timeline_cache(&mut self, timeline: TimelineName) {
        re_tracing::profile_function!(); // There can be A LOT of tiny allocations to drop.
        self.per_timeline.remove(&timeline);
    }

    /// Makes sure the internal transform index is up to date and outdated cache entries are discarded.
    ///
    /// This needs to be called once per frame prior to any transform propagation.
    /// (which is done by [`crate::TransformForest`])
    ///
    /// This will internally…
    /// * keep track of which child frames are influenced by which entity
    /// * create empty entries for where transforms may change over time (may happen conservatively - creating more entries than needed)
    ///     * this may invalidate previous entries at the same position
    /// * remove cached entries if chunks were GC'ed
    pub fn process_store_events<'a>(
        &mut self,
        events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>,
    ) {
        re_tracing::profile_function!();

        for event in events {
            // This doesn't maintain a collection of chunks that needs to be kept in sync 1:1 with
            // the store, rather it just keeps track of what entities have what properties, and for
            // that a delta chunk is all we need.
            let Some(delta_chunk) = event.delta_chunk() else {
                continue; // virtual event, we don't care
            };

            // Since entity paths lead to implicit frames, we have to prime our lookup table
            // with them even if this chunk doesn't have transform data.
            self.frame_id_registry
                .write()
                .register_all_frames_in_chunk(delta_chunk);

            let aspects = TransformAspect::transform_aspects_of(delta_chunk);
            if aspects.is_empty() {
                continue;
            }

            if event.is_deletion() {
                self.remove_chunk(delta_chunk, aspects);
            } else if delta_chunk.is_static() {
                self.add_static_chunk(delta_chunk, aspects);
            } else {
                self.add_temporal_chunk(delta_chunk, aspects);
            }
        }
    }

    fn add_temporal_chunk(&self, chunk: &re_chunk_store::Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!(format!(
            "{} rows, {}",
            chunk.num_rows(),
            chunk.entity_path()
        ));

        debug_assert!(!chunk.is_static());

        let static_timeline = self.static_timeline.read();
        let frame_id_registry = self.frame_id_registry.read();

        for timeline in chunk.timelines().keys() {
            // Skip timelines that haven't been requested yet (lazy initialization).
            let Some(per_timeline) = self.per_timeline.get(timeline) else {
                continue;
            };
            let mut per_timeline = per_timeline.write();

            per_timeline.add_temporal_chunk(
                chunk,
                aspects,
                *timeline,
                &static_timeline,
                &frame_id_registry,
            );
        }
    }

    fn add_static_chunk(&mut self, chunk: &re_chunk_store::Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        debug_assert!(chunk.is_static());

        let entity_path = chunk.entity_path();
        let place_holder_timeline = TimelineName::new("ignored for static chunk");

        let transform_child_frame_component =
            archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component = archetypes::Pinhole::descriptor_child_frame().component;

        let mut static_timeline = self.static_timeline.write();
        let frame_id_registry = self.frame_id_registry.read();

        // Add a static transform invalidation to affected child frames on ALL timelines.

        if aspects.contains(TransformAspect::Frame) {
            for (time, frame) in iter_child_frames_in_chunk(
                chunk,
                place_holder_timeline,
                transform_child_frame_component,
            ) {
                debug_assert_eq!(time, TimeInt::STATIC);

                let frame_transforms = static_timeline.get_or_create_tree_transforms_static(
                    entity_path,
                    frame,
                    &frame_id_registry,
                );
                frame_transforms.invalidate_transform_at(TimeInt::STATIC);

                #[cfg_attr(not(debug_assertions), expect(clippy::for_kv_map))]
                for (_timeline, per_timeline) in &mut self.per_timeline {
                    // Don't call `get_or_create_tree_transforms_temporal` here since we may not yet know a temporal entity that this is associated with.
                    // Also, this may be the first time we associate with a static entity instead which `get_or_create_tree_transforms_static` takes care of.
                    let mut per_timeline_guard = per_timeline.write();
                    let transforms = per_timeline_guard.get_or_create_tree_transforms_static(
                        entity_path,
                        frame,
                        &frame_id_registry,
                    );
                    transforms.invalidate_transform_at(TimeInt::STATIC);

                    // Entry might have been newly created. Have to ensure that its associated with the right timeline.
                    #[cfg(debug_assertions)]
                    {
                        transforms.timeline = Some(*_timeline);
                    }
                }
            }
        }
        if aspects.contains(TransformAspect::Pose) {
            let frame_transforms =
                static_timeline.get_or_create_pose_transforms_static(entity_path);
            frame_transforms.invalidate_at(TimeInt::STATIC);

            for per_timeline in self.per_timeline.values_mut() {
                per_timeline
                    .write()
                    .get_or_create_pose_transforms_temporal(entity_path, &static_timeline)
                    .invalidate_at(TimeInt::STATIC);
            }
        }
        if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
            for (time, frame) in iter_child_frames_in_chunk(
                chunk,
                place_holder_timeline,
                pinhole_child_frame_component,
            ) {
                debug_assert_eq!(time, TimeInt::STATIC);

                let frame_transforms = static_timeline.get_or_create_tree_transforms_static(
                    entity_path,
                    frame,
                    &frame_id_registry,
                );
                frame_transforms.invalidate_pinhole_projection_at(TimeInt::STATIC);

                #[cfg_attr(not(debug_assertions), expect(clippy::for_kv_map))]
                for (_timeline, per_timeline) in &mut self.per_timeline {
                    // Don't call `get_or_create_tree_transforms_temporal` here since we may not yet know a temporal entity that this is associated with.
                    // Also, this may be the first time we associate with a static entity instead which `get_or_create_tree_transforms_static` takes care of.
                    let mut per_timeline_guard = per_timeline.write();
                    let transforms = per_timeline_guard.get_or_create_tree_transforms_static(
                        entity_path,
                        frame,
                        &frame_id_registry,
                    );
                    transforms.invalidate_pinhole_projection_at(TimeInt::STATIC);

                    // Entry might have been newly created. Have to ensure that its associated with the right timeline.
                    #[cfg(debug_assertions)]
                    {
                        transforms.timeline = Some(*_timeline);
                    }
                }
            }
        }

        // Don't care about clears here, they don't have any effect for keeping track of changes when logged static.
    }

    fn remove_chunk(&mut self, chunk: &re_chunk_store::Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        // TODO(andreas): handle removal of static chunks?
        for timeline in chunk.timelines().keys() {
            let Some(per_timeline_rw) = self.per_timeline.get_mut(timeline) else {
                continue;
            };

            let mut per_timeline = per_timeline_rw.write();
            per_timeline.remove_chunk(chunk, aspects, *timeline);

            // Remove the entire timeline if it's empty.
            let is_empty = per_timeline.per_child_frame_transforms.is_empty();
            drop(per_timeline);
            if is_empty {
                self.per_timeline.remove(timeline);
            }
        }
    }
}
