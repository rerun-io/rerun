use crate::entity_to_source_frame_tracking::EntityToAffectedSources;
use crate::{
    TransformFrameIdHash,
    transform_aspect::TransformAspect,
    transform_queries::{
        query_and_resolve_instance_poses_at_entity, query_and_resolve_pinhole_projection_at_entity,
    },
};
use ahash::HashMap;
use glam::Affine3A;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::{Chunk, LatestAtQuery};
use re_entity_db::EntityDb;
use re_log_types::external::re_types_core::ArrowString;
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_types::{
    Archetype as _, ArchetypeName,
    archetypes::{self},
    components::{self},
};
use std::collections::{BTreeMap, BTreeSet, hash_map::Entry};
use std::ops::Range;
use vec1::smallvec_v1::SmallVec1;

/// Resolves all transform relationship defining components to affine transforms for fast lookup.
///
/// It only handles resulting transforms individually to each frame connection, not how these transforms propagate in the tree.
/// For transform tree propagation see [`crate::TransformForest`].
///
/// There are different kinds of transforms handled here:
/// * [`archetypes::Transform3D`]
///   Tree transforms that should propagate in the tree (via [`crate::TransformForest`]).
/// * [`archetypes::InstancePoses3D`]
///   Instance poses that should be applied to the tree transforms (via [`crate::TransformForest`]) but not propagate.
/// * [`components::PinholeProjection`] and [`components::ViewCoordinates`]
///   Pinhole projections & associated view coordinates used for visualizing cameras in 3D and embedding 2D in 3D
pub struct TransformResolutionCache {
    per_timeline: HashMap<TimelineName, CachedTransformsForTimeline>,
    static_timeline: CachedTransformsForTimeline,
}

impl Default for TransformResolutionCache {
    #[inline]
    fn default() -> Self {
        Self {
            per_timeline: Default::default(),
            // `CachedTransformsForTimeline` intentionally doesn't implement Default to not accidentally create it without considering static transforms.
            static_timeline: CachedTransformsForTimeline {
                per_entity_affected_sources: Default::default(),
                per_source_frame_transforms: Default::default(),
                recursive_clears: Default::default(), // Unused for static timeline.
            },
        }
    }
}

/// A transform from a source frame to a target frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceToTargetTransform {
    /// The frame we're transforming into.
    pub target: TransformFrameIdHash,

    /// The transform from the source frame to the target frame.
    pub transform: Affine3A,
}

/// Maps entity paths to [`EntityToAffectedSources`] datastructures.
///
/// See [`EntityToAffectedSources`] for details.
#[derive(Default, Clone)]
struct PerEntityAffectedSources(IntMap<EntityPath, EntityToAffectedSources>);

impl PerEntityAffectedSources {
    fn get_or_create_for(&mut self, entity_path: &EntityPath) -> &mut EntityToAffectedSources {
        self.0
            .entry(entity_path.clone())
            .or_insert_with(|| EntityToAffectedSources::new(entity_path))
    }
}

impl std::ops::Deref for PerEntityAffectedSources {
    type Target = IntMap<EntityPath, EntityToAffectedSources>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PerEntityAffectedSources {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Cached transforms for a single timeline.
///
/// Includes any static transforms that may apply globally.
/// Therefore, this can't be trivially constructed.
pub struct CachedTransformsForTimeline {
    /// Maps entity paths to [`EntityToAffectedSources`] datastructures.
    ///
    /// This allows us to keep track of which incoming (or removed) transform events on entities, apply to which
    /// source-transform at which time.
    per_entity_affected_sources: PerEntityAffectedSources,

    /// Transforms information for each source frame to a target frame over time.
    per_source_frame_transforms: IntMap<TransformFrameIdHash, TransformsForSourceFrame>,

    // We need to keep track of all recursive clears that ever happened and when.
    // Otherwise, new incoming entities may not correctly change their transform at the time of clear.
    recursive_clears: IntMap<EntityPath, BTreeSet<TimeInt>>,
}

impl CachedTransformsForTimeline {
    fn new(timeline: &TimelineName, static_transforms: &Self) -> Self {
        Self {
            per_entity_affected_sources: static_transforms.per_entity_affected_sources.clone(),
            per_source_frame_transforms: static_transforms
                .per_source_frame_transforms
                .iter()
                .map(|(transform_frame, static_transforms)| {
                    (
                        *transform_frame,
                        TransformsForSourceFrame::new_for_new_empty_timeline(
                            *timeline,
                            static_transforms,
                        ),
                    )
                })
                .collect(),
            recursive_clears: IntMap::default(),
        }
    }

    fn add_recursive_clears(
        &mut self,
        recursively_cleared_entity_path: &EntityPath,
        mut times: BTreeSet<TimeInt>,
    ) {
        re_tracing::profile_function!();

        // Add clears to all existing entities that it affects.
        for (cleared_path, affected_source_per_start_time) in
            &mut self.per_entity_affected_sources.iter_mut()
        {
            if !cleared_path.starts_with(recursively_cleared_entity_path) {
                continue;
            }

            for time in &times {
                // Which sources are affected by this clear?
                let Some((_, sources)) = affected_source_per_start_time
                    .range_starts
                    .range(..=time)
                    .next_back()
                else {
                    debug_assert!(
                        false,
                        "For any given time, there should always be a time in entity_source_ranges that is <= time."
                    );
                    continue;
                };

                // Insert clears into the per-source datastructures.
                for source in sources {
                    if let Some(frame_transforms) = self.per_source_frame_transforms.get_mut(source)
                    {
                        frame_transforms.add_clear(*time, cleared_path);
                    } else {
                        debug_panic_missing_source_transforms_for_update_on_entity(
                            cleared_path,
                            *source,
                        );
                    }
                }
            }
        }

        // Store for future reference.
        self.recursive_clears
            .entry(recursively_cleared_entity_path.clone())
            .or_default()
            .append(&mut times);
    }

    fn remove_recursive_clears(
        &mut self,
        recursively_cleared_entity_path: &EntityPath,
        times: &BTreeSet<TimeInt>,
    ) {
        if let Entry::Occupied(mut clear_entry) = self
            .recursive_clears
            .entry(recursively_cleared_entity_path.clone())
        {
            *clear_entry.get_mut() = clear_entry.get().difference(times).copied().collect();

            if clear_entry.get().is_empty() {
                clear_entry.remove();
            }
        }

        // Removing clears from `self.per_source_frame_transforms` is not critical since leftover cache entries won't change outcomes.
    }

    /// Returns all transforms for a given source frame.
    #[inline]
    pub fn frame_transforms(
        &mut self,
        source_frame: TransformFrameIdHash,
    ) -> Option<&mut TransformsForSourceFrame> {
        self.per_source_frame_transforms.get_mut(&source_frame)
    }
}

/// Maps from archetype to resolved pose transform.
///
/// If there's a concrete archetype in here, the mapped values are the full resolved pose transform.
///
/// `TransformResolutionCache` doesn't do tree propagation, however (!!!) there's a mini-tree in here that we already fully apply:
/// `InstancePose3D` is applied on top of concrete archetype poses.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PoseTransformArchetypeMap {
    /// Iff there's a concrete archetype in here, the mapped values are the full resolved pose transform.
    // TODO(andreas): use some kind of small map? Vec of tuples might already be more appropriate?
    pub instance_from_archetype_poses_per_archetype:
        IntMap<ArchetypeName, SmallVec1<[Affine3A; 1]>>,

    /// Resolved transforms for the instance poses archetype if any.
    pub instance_from_poses: Vec<Affine3A>,
}

impl PoseTransformArchetypeMap {
    #[cfg(test)]
    #[inline]
    fn get(&self, archetype: ArchetypeName) -> &[Affine3A] {
        self.instance_from_archetype_poses_per_archetype
            .get(&archetype)
            .map_or(&self.instance_from_poses, |v| v.as_slice())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TransformEntry<T> {
    /// The entity path that produced information about this transform at this time.
    ///
    /// Note that it is user-data error if there's several entities producing data for the same source at the same time.
    /// (the entity that holds information about a source->target transform can however change over time!)
    // TODO(andreas): We decided that for any given source the entity may not change over time except for static. Meaning that we can put this into a lookup table instead.
    entity_path: EntityPath,

    /// The cached transform value.
    value: CachedTransformValue<T>,
}

impl<T> TransformEntry<T> {
    fn new(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            value: CachedTransformValue::Invalidated,
        }
    }

    fn new_cleared(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            value: CachedTransformValue::Cleared,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CachedTransformValue<T> {
    /// Cache is invalidated, we don't know what state we're in.
    Invalidated,

    /// There's a transform at this time.
    Resident(T),

    /// The value has been cleared out at this time.
    Cleared,
}

type FrameTransformTimeMap = BTreeMap<TimeInt, TransformEntry<SourceToTargetTransform>>;

type PoseTransformTimeMap = BTreeMap<TimeInt, TransformEntry<PoseTransformArchetypeMap>>;

type PinholeProjectionMap = BTreeMap<TimeInt, TransformEntry<ResolvedPinholeProjection>>;

/// Cached transforms from a single source frame to a target frame over time.
///
/// Incorporates any static transforms that may apply to this entity.
///
/// Time points are conservative: it can happen that we generate new events (==cache slots) despite no change
/// occurring for this source frame.
/// However, we mustn't ever note down timepoints at which the given source frame is not "active" on its entity.
/// Doing so would mean that queries using `re_query` yield information about a _different_ source
/// which we then can't add to the cache entries of the current source.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformsForSourceFrame {
    // Is None if this is about the "static timeline".
    #[cfg(debug_assertions)]
    timeline: Option<TimelineName>,

    /// There can be only a single target at any point in time, but it may change over time.
    /// Whenever it changes, the previous target frame is no longer reachable.
    frame_transforms: FrameTransformTimeMap,

    pose_transforms: Option<Box<PoseTransformTimeMap>>,
    pinhole_projections: Option<Box<PinholeProjectionMap>>,
}

impl TransformsForSourceFrame {
    /// Invalidates all transforms for the given aspects starting at the given time `min_time` (inclusive) and adds new invalidated times.
    ///
    /// [`TransformAspect::Clear`] causes all types of transforms to be invalidated and being added to.
    pub fn insert_invalidated_transform_events<I: Iterator<Item = TimeInt>>(
        &mut self,
        aspects: TransformAspect,
        min_time: TimeInt,
        get_new_invalidated_times: impl Fn() -> I,
        entity_path: &EntityPath,
    ) {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms,
            pose_transforms,
            pinhole_projections,
        } = self;

        // This invalidates any time _after_ the first event in this chunk.
        // (e.g. if a rotation is added prior to translations later on,
        // then the resulting transforms at those translations change as well for latest-at queries)

        // Min time is conservative - technically we want to check this for each component individually,
        // but using the same for all is fine as it rarely matters.
        // (it may produce some false positive transform updates)

        // TODO(andreas): this is clearly _too_ conservative for long recordings.
        // We'd like to know all points in time when a transform is fully "shadowed", so we don't have to invalidate as aggressively.

        if aspects.intersects(TransformAspect::Frame | TransformAspect::Clear) {
            // Invalidate existing transforms after min_time (rationale see above).
            for (_, transform) in frame_transforms.range_mut(min_time..) {
                *transform = TransformEntry::new(transform.entity_path.clone());
            }

            // Add new invalidated transforms.
            frame_transforms.extend(
                get_new_invalidated_times()
                    .map(|time| (time, TransformEntry::new(entity_path.clone()))),
            );
        }

        if aspects.intersects(TransformAspect::Pose | TransformAspect::Clear) {
            let pose_transforms = pose_transforms.get_or_insert_with(Box::default);

            // Invalidate existing transforms after min_time (rationale see above).
            for (_, transform) in pose_transforms.range_mut(min_time..) {
                *transform = TransformEntry::new(transform.entity_path.clone());
            }

            // Add new invalidated transforms.
            pose_transforms.extend(
                get_new_invalidated_times()
                    .map(|time| (time, TransformEntry::new(entity_path.clone()))),
            );
        }

        if aspects.intersects(TransformAspect::PinholeOrViewCoordinates | TransformAspect::Clear) {
            let pinhole_projections = pinhole_projections.get_or_insert_with(Box::default);

            // Invalidate existing transforms after min_time (rationale see above).
            for (_, transform) in pinhole_projections.range_mut(min_time..) {
                *transform = TransformEntry::new(transform.entity_path.clone());
            }

            // Add new invalidated transforms.
            pinhole_projections.extend(
                get_new_invalidated_times()
                    .map(|time| (time, TransformEntry::new(entity_path.clone()))),
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPinholeProjection {
    /// The target frame of the pinhole projection.
    pub target: TransformFrameIdHash,

    pub image_from_camera: components::PinholeProjection,

    pub resolution: Option<components::Resolution>,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`archetypes::Pinhole::DEFAULT_CAMERA_XYZ`].
    pub view_coordinates: components::ViewCoordinates,
}

impl TransformsForSourceFrame {
    fn new(
        source_frame: TransformFrameIdHash,
        _timeline: TimelineName,
        static_timeline: &CachedTransformsForTimeline,
    ) -> Self {
        let mut frame_transforms = BTreeMap::new();
        let mut pose_transforms = None;
        let mut pinhole_projections = None;

        if let Some(static_transforms) = static_timeline
            .per_source_frame_transforms
            .get(&source_frame)
        {
            frame_transforms = static_transforms.frame_transforms.clone();
            pose_transforms = static_transforms.pose_transforms.clone();
            pinhole_projections = static_transforms.pinhole_projections.clone();
        }

        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            pose_transforms,
            frame_transforms,
            pinhole_projections,
        }
    }

    fn new_for_new_empty_timeline(_timeline: TimelineName, static_timeline_entry: &Self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            ..static_timeline_entry.clone()
        }
    }

    fn new_empty() -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: None,
            frame_transforms: BTreeMap::new(),
            pose_transforms: None,
            pinhole_projections: None,
        }
    }

    /// Inserts a cleared transform for the given times.
    fn add_clear(&mut self, time: TimeInt, entity_path: &EntityPath) {
        self.frame_transforms
            .insert(time, TransformEntry::new_cleared(entity_path.clone()));
        self.pose_transforms
            .get_or_insert(Default::default())
            .insert(time, TransformEntry::new_cleared(entity_path.clone()));
        self.pinhole_projections
            .get_or_insert(Default::default())
            .insert(time, TransformEntry::new_cleared(entity_path.clone()));
    }

    /// Removes any events at a given time (if any).
    fn remove_event_at(&mut self, time: TimeInt) {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms,
            pose_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.remove(&time);
        if let Some(pose_transforms) = pose_transforms.as_mut() {
            pose_transforms.remove(&time);
        }
        if let Some(pinhole_projections) = &mut pinhole_projections.as_mut() {
            pinhole_projections.remove(&time);
        }
    }

    /// Removes all events in a given range and writes them to `destination`.
    fn remove_events_in_range(&mut self, range: Range<TimeInt>, destination: &mut Self) {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms,
            pose_transforms,
            pinhole_projections,
        } = self;

        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms: dst_frame_transforms,
            pose_transforms: dst_pose_transforms,
            pinhole_projections: dst_pinhole_projections,
        } = destination;

        frame_transforms.retain(|time, transform| {
            if !range.contains(time) {
                return true;
            }
            dst_frame_transforms.insert(*time, transform.clone());
            false
        });

        if let Some(pose_transforms) = pose_transforms {
            let dst_pose_transforms = dst_pose_transforms.get_or_insert_default();

            pose_transforms.retain(|time, transform| {
                if !range.contains(time) {
                    return true;
                }
                dst_pose_transforms.insert(*time, transform.clone());
                false
            });
        }

        if let Some(pinhole_projections) = pinhole_projections {
            let dst_pinhole_projections = dst_pinhole_projections.get_or_insert_default();

            pinhole_projections.retain(|time, transform| {
                if !range.contains(time) {
                    return true;
                }
                dst_pinhole_projections.insert(*time, transform.clone());
                false
            });
        }
    }

    fn insert_all_events_of(&mut self, other: &Self) {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms,
            pose_transforms,
            pinhole_projections,
        } = self;

        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            frame_transforms: src_frame_transforms,
            pose_transforms: src_pose_transforms,
            pinhole_projections: src_pinhole_projections,
        } = other;

        frame_transforms.extend(
            src_frame_transforms
                .iter()
                .map(|(time, transform)| (*time, transform.clone())),
        );
        if let Some(src_pose_transforms) = src_pose_transforms {
            pose_transforms.get_or_insert_default().extend(
                src_pose_transforms
                    .iter()
                    .map(|(time, transform)| (*time, transform.clone())),
            );
        }
        if let Some(src_pinhole_projections) = src_pinhole_projections {
            pinhole_projections.get_or_insert_default().extend(
                src_pinhole_projections
                    .iter()
                    .map(|(time, transform)| (*time, transform.clone())),
            );
        }
    }

    #[inline]
    pub fn latest_at_transform(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<SourceToTargetTransform> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let (time_of_last_update_to_this_source, frame_transform) = self
            .frame_transforms
            .range_mut(..query.at().inc())
            .next_back()?;

        match &frame_transform.value {
            CachedTransformValue::Resident(transform) => Some(transform.clone()),
            CachedTransformValue::Cleared => None,
            CachedTransformValue::Invalidated => {
                let transform = query_and_resolve_tree_transform_at_entity(
                    &frame_transform.entity_path,
                    entity_db,
                    // Do NOT use the original query time since that may give us information about a different source frame!
                    &LatestAtQuery::new(query.timeline(), *time_of_last_update_to_this_source),
                );

                frame_transform.value = match &transform {
                    Some(transform) => CachedTransformValue::Resident(transform.clone()),
                    None => CachedTransformValue::Cleared,
                };
                transform
            }
        }
    }

    #[inline]
    pub fn latest_at_instance_poses(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<&PoseTransformArchetypeMap> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let pose_transform = self
            .pose_transforms
            .as_mut()?
            .range_mut(..query.at().inc())
            .next_back()?
            .1;

        // Separate check to work around borrow checker issue.
        if pose_transform.value == CachedTransformValue::Invalidated {
            pose_transform.value =
                CachedTransformValue::Resident(query_and_resolve_instance_poses_at_entity(
                    &pose_transform.entity_path,
                    entity_db,
                    query,
                ));
        }

        match &pose_transform.value {
            CachedTransformValue::Resident(transform) => Some(transform),
            CachedTransformValue::Cleared => None,
            CachedTransformValue::Invalidated => unreachable!("Just made transform cache-resident"),
        }
    }

    #[inline]
    pub fn latest_at_pinhole(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<&ResolvedPinholeProjection> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let pinhole_projection = self
            .pinhole_projections
            .as_mut()?
            .range_mut(..query.at().inc())
            .next_back()?
            .1;

        // Separate check to work around borrow checker issue.
        if pinhole_projection.value == CachedTransformValue::Invalidated {
            let transform = query_and_resolve_pinhole_projection_at_entity(
                &pinhole_projection.entity_path,
                entity_db,
                query,
            );

            pinhole_projection.value = match &transform {
                Some(transform) => CachedTransformValue::Resident(transform.clone()),
                None => CachedTransformValue::Cleared,
            };
        }

        match &pinhole_projection.value {
            CachedTransformValue::Resident(transform) => Some(transform),
            CachedTransformValue::Cleared => None,
            CachedTransformValue::Invalidated => unreachable!("Just made transform cache-resident"),
        }
    }
}

impl TransformResolutionCache {
    /// Accesses the transform component tracking data for a given timeline.
    ///
    /// Returns `None` if the timeline doesn't have any transforms at all.
    #[inline]
    pub fn transforms_for_timeline(
        &mut self,
        timeline: TimelineName,
    ) -> &mut CachedTransformsForTimeline {
        self.per_timeline
            .get_mut(&timeline)
            .unwrap_or(&mut self.static_timeline)
    }

    /// Makes sure the internal transform index is up to date and outdated cache entries are discarded.
    ///
    /// This needs to be called once per frame prior to any transform propagation.
    /// (which is done by [`crate::TransformForest`])
    ///
    /// This will internally…
    /// * keep track of which source frames are influenced by which entity
    /// * invalidate cache entries if needed (may happen conservatively - potentially invalidating more than needed)
    /// * create empty entries for where transforms may change over time (may happen conservatively - creating more entries than needed)
    /// * remove cached entries if chunks were GC'ed
    ///
    /// See also [`Self::add_chunks`].
    pub fn process_store_events<'a>(
        &mut self,
        events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>,
    ) {
        re_tracing::profile_function!();

        // TODO(andreas): We eagerly index for all timelines even if they're never used.
        // Instead, we should do so lazily when results for a timeline are queried.

        for event in events {
            let aspects = TransformAspect::transform_aspects_of(&event.chunk);
            if aspects.is_empty() {
                continue;
            }

            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                self.remove_chunk(&event.chunk, aspects);
            } else if event.diff.chunk.is_static() {
                self.add_static_chunk(&event.chunk, aspects);
            } else {
                self.add_temporal_chunk(&event.chunk, aspects);
            }
        }
    }

    /// Adds chunks to the transform cache.
    ///
    /// This will internally…
    /// * keep track of which source frames are influenced by which entity
    /// * invalidate cache entries if needed (may happen conservatively - potentially invalidating more than needed)
    /// * create empty entries for where transforms may change over time (may happen conservatively - creating more entries than needed)
    ///
    /// See also [`Self::process_store_events`].
    pub fn add_chunks<'a>(&mut self, chunks: impl Iterator<Item = &'a std::sync::Arc<Chunk>>) {
        re_tracing::profile_function!();

        // TODO(andreas): We eagerly index for all timelines even if they're never used.
        // Instead, we should do so lazily when results for a timeline are queried.

        for chunk in chunks {
            let aspects = TransformAspect::transform_aspects_of(chunk);
            if aspects.is_empty() {
                continue;
            }

            if chunk.is_static() {
                self.add_static_chunk(chunk, aspects);
            } else {
                self.add_temporal_chunk(chunk, aspects);
            }
        }
    }

    fn add_temporal_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        debug_assert!(!chunk.is_static());

        let entity_path = chunk.entity_path();

        for (timeline, time_column) in chunk.timelines() {
            let per_timeline = self.per_timeline.entry(*timeline).or_insert_with(|| {
                CachedTransformsForTimeline::new(timeline, &self.static_timeline)
            });

            // Keeps track which of the sources are new for this entity.
            let mut sources_affected_by_this_entity_for_first_time = IntSet::default();

            let affected_sources = per_timeline
                .per_entity_affected_sources
                .entry(entity_path.clone())
                .or_insert_with(|| {
                    sources_affected_by_this_entity_for_first_time
                        .insert(TransformFrameIdHash::from_entity_path(entity_path));
                    EntityToAffectedSources::new(entity_path)
                });

            // First, update the list of when which source is "active" for this entity in case this chunk mentions any sources.
            for (start_time, sources) in iter_source_frames_in_chunk(chunk, *timeline) {
                let sources = active_source_array_from_string_slice(entity_path, &sources);
                sources_affected_by_this_entity_for_first_time.extend(
                    sources
                        .iter()
                        .filter(|source| !affected_sources.all_sources.contains(source)),
                );

                let (changed_range, previous_sources) =
                    affected_sources.insert_range_start(start_time, sources.clone());

                // Since (by convention) only this entity can affect `previous_sources`, we have to drop all their events in the `changed_range`!
                let mut moved_events = TransformsForSourceFrame::new_empty();
                for previous_source_frame in &previous_sources {
                    let Some(frame_transforms) = per_timeline
                        .per_source_frame_transforms
                        .get_mut(previous_source_frame)
                    else {
                        // No events on this source, so nothing to remove!
                        continue;
                    };
                    // Since (by convention) only this entity can affect `previous_sources`, we have to move all their events in the `changed_range` to the new range.
                    frame_transforms
                        .remove_events_in_range(changed_range.clone(), &mut moved_events);
                }
                // …and add them to the new sources!
                for new_source_frame in sources {
                    per_timeline
                        .per_source_frame_transforms
                        .entry(new_source_frame)
                        .or_insert_with(|| {
                            TransformsForSourceFrame::new(
                                new_source_frame,
                                *timeline,
                                &self.static_timeline,
                            )
                        })
                        .insert_all_events_of(&moved_events);
                }
            }

            // Now that our map of active sources is up to date, we can insert "event points" (invalidated cache entries)
            // into the respective per-source data structures.
            for (time_range, source_frames) in
                affected_sources.iter_ranges(time_column.time_range())
            {
                // We now look only at the times in the time column that are relevant for this child-frame.
                // Note that there may be more times than actual relevant updates, but crucially, all queries
                // to the current entity path yield information about the sources in `source_frames`.
                let times_with_potential_update = time_column
                    .times()
                    // TODO(andreas): For sorted time columns we could speed this up a bit.
                    .filter(|time| time_range.contains(time))
                    .collect_vec();

                // Note down that all these source frames were updated at the given times.
                for source_frame in source_frames {
                    // Invalidate all frames for this source frame.
                    let frame_transforms = per_timeline
                        .per_source_frame_transforms
                        .entry(*source_frame)
                        .or_insert_with(|| {
                            TransformsForSourceFrame::new(
                                *source_frame,
                                *timeline,
                                &self.static_timeline,
                            )
                        });

                    frame_transforms.insert_invalidated_transform_events(
                        aspects,
                        time_range.start,
                        || times_with_potential_update.iter().copied(),
                        entity_path,
                    );

                    // If we've never seen this entity update these source-frames,
                    // we have to make sure that we take recursive clears into account.
                    if sources_affected_by_this_entity_for_first_time.contains(source_frame) {
                        let mut ancestor = entity_path.clone();
                        loop {
                            if let Some(cleared_times) =
                                per_timeline.recursive_clears.get(&ancestor)
                            {
                                for cleared_time in cleared_times {
                                    if time_range.contains(cleared_time) {
                                        frame_transforms.add_clear(*cleared_time, entity_path);
                                    }
                                }
                            }

                            match ancestor.parent() {
                                Some(parent) => ancestor = parent,
                                None => break,
                            }
                        }
                    }
                }
            }

            // Keep track of recursive clears.
            if aspects.contains(TransformAspect::Clear) {
                re_tracing::profile_scope!("check for recursive clears");

                let component = archetypes::Clear::descriptor_is_recursive().component;

                let recursively_cleared_times = chunk
                    .iter_component_indices(*timeline, component)
                    .zip(chunk.iter_slices::<bool>(component))
                    .filter_map(|((time, _row_id), bool_slice)| {
                        bool_slice
                            .values()
                            .first()
                            .and_then(|is_recursive| (*is_recursive != 0).then_some(time))
                    })
                    .collect::<BTreeSet<_>>();

                if !recursively_cleared_times.is_empty() {
                    per_timeline.add_recursive_clears(entity_path, recursively_cleared_times);
                }
            }
        }
    }

    fn add_static_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        debug_assert!(chunk.is_static());

        let entity_path = chunk.entity_path();
        let fallback_sources = [TransformFrameIdHash::from_entity_path(entity_path)];

        let affected_sources = self
            .static_timeline
            .per_entity_affected_sources
            .get_or_create_for(entity_path);

        // Note down that for these source frames we may have new static transforms.
        let source_frames = source_frames_in_static_chunk(chunk);
        let source_frames =
            active_source_array_from_string_slice(entity_path, &source_frames.unwrap_or_default());
        {
            let (changed_range, previous_sources) =
                affected_sources.insert_range_start(TimeInt::STATIC, source_frames.clone());
            debug_assert_eq!(changed_range, TimeInt::STATIC..TimeInt::STATIC);

            if previous_sources != source_frames
                && previous_sources.as_slice() != fallback_sources.as_slice()
            {
                for per_timeline_transforms in &mut self.per_timeline.values_mut() {
                    // Propagate the new static sources to `entity_source_ranges` on all timelines.
                    per_timeline_transforms
                        .per_entity_affected_sources
                        .get_or_create_for(entity_path)
                        .insert_range_start(TimeInt::STATIC, source_frames.clone());

                    // Invalidate the static status on the previous sources.
                    for previous_source_frame in &previous_sources {
                        if let Some(frame_transform) = per_timeline_transforms
                            .per_source_frame_transforms
                            .get_mut(previous_source_frame)
                        {
                            frame_transform.remove_event_at(TimeInt::STATIC);
                        }
                    }
                }
            }
        }
        debug_assert_eq!(
            affected_sources.range_starts.len(),
            1,
            "There should be only information about the static source frame"
        );

        // Adding a static transform invalidates affected source frames on ALL timelines, since the resulting transforms at all times may be different now.
        // TODO(andreas): This is too conservative for long recordings - we should know when a static transform is fully "shadowed", so we don't have to invalidate as aggressively.
        // Furthermore, since we want to incorporate the static transforms into all timelines, we have to add this event to all timelines.
        for source_frame in source_frames {
            // Note down the events/invalidations on the static timeline itself.
            self.static_timeline
                .per_source_frame_transforms
                .entry(source_frame)
                .or_insert_with(TransformsForSourceFrame::new_empty)
                .insert_invalidated_transform_events(
                    aspects,
                    TimeInt::STATIC,
                    || std::iter::once(TimeInt::STATIC),
                    entity_path,
                );

            for (timeline, per_timeline_transforms) in &mut self.per_timeline {
                let entity_transforms = per_timeline_transforms
                    .per_source_frame_transforms
                    .entry(source_frame)
                    .or_insert_with(|| {
                        // Need to add an entry now if there wasn't one before.
                        // Also note that the static transforms we use to construct this might touch on aspects that aren't invalidated, so it's still important to pass that in.
                        TransformsForSourceFrame::new(
                            source_frame,
                            *timeline,
                            &self.static_timeline,
                        )
                    });

                entity_transforms.insert_invalidated_transform_events(
                    aspects,
                    TimeInt::STATIC,
                    || std::iter::once(TimeInt::STATIC),
                    entity_path,
                );
            }
        }

        // Don't care about clears here, they don't have any effect for keeping track of changes when logged static.
    }

    fn remove_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        let entity_path = chunk.entity_path();

        // Note that we ignore static timelines for removal.
        for (timeline, time_column) in chunk.timelines() {
            let Some(per_timeline) = self.per_timeline.get_mut(timeline) else {
                continue;
            };

            // Remove any affected recursive clears.
            if aspects.contains(TransformAspect::Clear) {
                re_tracing::profile_scope!("check for recursive clears");

                let component = archetypes::Clear::descriptor_is_recursive().component;

                let recursively_cleared_times = chunk
                    .iter_component_indices(*timeline, component)
                    .zip(chunk.iter_slices::<bool>(component))
                    .filter_map(|((time, _row_id), bool_slice)| {
                        bool_slice
                            .values()
                            .first()
                            .and_then(|is_recursive| (*is_recursive != 0).then_some(time))
                    })
                    .collect::<BTreeSet<_>>();

                if !recursively_cleared_times.is_empty() {
                    per_timeline.remove_recursive_clears(entity_path, &recursively_cleared_times);
                }
            }

            // Remove existing data.
            if let Some(affected_sources) = per_timeline
                .per_entity_affected_sources
                .get_mut(entity_path)
            {
                for (time_range, source_frames) in
                    affected_sources.iter_ranges(time_column.time_range())
                {
                    for source in source_frames {
                        let Some(source_transforms) =
                            per_timeline.per_source_frame_transforms.get_mut(source)
                        else {
                            debug_panic_missing_source_transforms_for_update_on_entity(
                                entity_path,
                                *source,
                            );
                            continue;
                        };

                        // Remove from our record of where this entity updates things.
                        for time in time_column.times() {
                            // Only if this entity actually had an update for a given source at a time, do we have to remove transforms from that source.
                            if !time_range.contains(&time) {
                                continue;
                            }

                            if aspects.contains(TransformAspect::Frame) {
                                source_transforms.frame_transforms.remove(&time);
                            }
                            if aspects.contains(TransformAspect::Pose)
                                && let Some(pose_transforms) =
                                    &mut source_transforms.pose_transforms
                            {
                                pose_transforms.remove(&time);
                            }
                            if aspects.contains(TransformAspect::PinholeOrViewCoordinates)
                                && let Some(pinhole_projections) =
                                    &mut source_transforms.pinhole_projections
                            {
                                pinhole_projections.remove(&time);
                            }
                        }

                        // Remove source entry if it's empty.
                        if source_transforms.frame_transforms.is_empty()
                            && source_transforms
                                .pose_transforms
                                .as_ref()
                                .is_none_or(|pose_transforms| pose_transforms.is_empty())
                            && source_transforms
                                .pinhole_projections
                                .as_ref()
                                .is_none_or(|pinhole_projections| pinhole_projections.is_empty())
                        {
                            per_timeline.per_source_frame_transforms.remove(source);
                        }
                    }
                }

                // TODO(andreas): Remove empty source update mentions.
            }

            // Remove entire timeline if it's empty.
            if per_timeline.per_source_frame_transforms.is_empty() {
                self.per_timeline.remove(timeline);
            }
        }
    }
}

fn debug_panic_missing_source_transforms_for_update_on_entity(
    entity_path: &EntityPath,
    source: TransformFrameIdHash,
) {
    // There was no actual transform change for this source frame after all.
    assert!(
        !cfg!(debug_assertions),
        "DEBUG ASSERTION: Internally inconsistent state: entity {entity_path:?} had updates for source frame {source:?} but no transforms for that source frame were found. Please report this as a bug."
    );
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns `None`.
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
fn query_and_resolve_tree_transform_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<SourceToTargetTransform> {
    // TODO(RR-2799): Output more than one target at once, doing the usual clamping - means probably we can merge a lot of code here with instance poses!
    // TODO(andreas): Filter out styling components.
    let results = entity_db.latest_at(
        query,
        entity_path,
        archetypes::Transform3D::all_component_identifiers(),
    );
    if results.components.is_empty() {
        return None;
    }

    let target = results
        .component_mono_quiet::<components::TransformFrameId>(
            archetypes::Transform3D::descriptor_target_frame().component,
        )
        .map_or_else(
            || {
                TransformFrameIdHash::from_entity_path(
                    &entity_path.parent().unwrap_or(EntityPath::root()),
                )
            },
            |frame_id| TransformFrameIdHash::new(&frame_id),
        );

    let mut transform = Affine3A::IDENTITY;

    // It's an error if there's more than one component. Warn in that case.
    let mono_log_level = re_log::Level::Warn;

    // The order of the components here is important and checked by `debug_assert_transform_field_order`
    if let Some(translation) = results.component_mono_with_log_level::<components::Translation3D>(
        archetypes::Transform3D::descriptor_translation().component,
        mono_log_level,
    ) {
        transform = Affine3A::from(translation);
    }
    if let Some(axis_angle) = results
        .component_mono_with_log_level::<components::RotationAxisAngle>(
            archetypes::Transform3D::descriptor_rotation_axis_angle().component,
            mono_log_level,
        )
    {
        if let Ok(axis_angle) = Affine3A::try_from(axis_angle) {
            transform *= axis_angle;
        } else {
            return None;
        }
    }
    if let Some(quaternion) = results.component_mono_with_log_level::<components::RotationQuat>(
        archetypes::Transform3D::descriptor_quaternion().component,
        mono_log_level,
    ) {
        if let Ok(quaternion) = Affine3A::try_from(quaternion) {
            transform *= quaternion;
        } else {
            return None;
        }
    }
    if let Some(scale) = results.component_mono_with_log_level::<components::Scale3D>(
        archetypes::Transform3D::descriptor_scale().component,
        mono_log_level,
    ) {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            return None;
        }
        transform *= Affine3A::from(scale);
    }
    if let Some(mat3x3) = results.component_mono_with_log_level::<components::TransformMat3x3>(
        archetypes::Transform3D::descriptor_mat3x3().component,
        mono_log_level,
    ) {
        let affine_transform = Affine3A::from(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            return None;
        }
        transform *= affine_transform;
    }

    if results.component_mono_with_log_level::<components::TransformRelation>(
        archetypes::Transform3D::descriptor_relation().component,
        mono_log_level,
    ) == Some(components::TransformRelation::ChildFromParent)
    {
        let determinant = transform.matrix3.determinant();
        if determinant != 0.0 && determinant.is_finite() {
            transform = transform.inverse();
        } else {
            // All "regular invalid" transforms should have been caught.
            // So ending up here means something else went wrong?
            re_log::warn_once!(
                "Failed to express child-from-parent transform at {} since it wasn't invertible",
                entity_path,
            );
        }
    }

    Some(SourceToTargetTransform { transform, target })
}

/// Iterates over all source frames that are in a chunk.
pub fn iter_source_frames_in_chunk(
    chunk: &Chunk,
    timeline: TimelineName,
) -> impl Iterator<Item = (TimeInt, Vec<ArrowString>)> {
    // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses, we instead use whatever is on `Transform3D`.
    let source_frame_component = archetypes::Transform3D::descriptor_source_frame().component;

    itertools::izip!(
        chunk
            .iter_component_indices(timeline, source_frame_component)
            .map(|(t, _)| t),
        chunk.iter_slices::<String>(source_frame_component),
    )
}

/// Iterates over all source frames that are in a chunk.
pub fn source_frames_in_static_chunk(chunk: &Chunk) -> Option<Vec<ArrowString>> {
    debug_assert!(chunk.is_static());

    // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses, we instead use whatever is on `Transform3D`.
    let source_frame_component = archetypes::Transform3D::descriptor_source_frame().component;

    chunk.iter_slices::<String>(source_frame_component).next()
}

/// Given a slice of arrow strings representing currently active sources, retrieve the list of active source frame hashes.
///
/// If there are no sources, this returns the implicit source since this one is active if nothing else was specified.
fn active_source_array_from_string_slice(
    entity_path: &EntityPath,
    sources: &[ArrowString],
) -> SmallVec1<[TransformFrameIdHash; 1]> {
    SmallVec1::try_from_smallvec(
        sources
            .iter()
            .map(|s| TransformFrameIdHash::from_str(s.as_str()))
            .collect(),
    )
    .unwrap_or_else(|_| {
        // Insert the implicit frame if the list was empty.
        SmallVec1::from_array_const([TransformFrameIdHash::from_entity_path(entity_path)])
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use super::*;
    use re_chunk_store::{
        Chunk, ChunkStore, ChunkStoreEvent, ChunkStoreSubscriberHandle, GarbageCollectionOptions,
        PerStoreChunkSubscriber, RowId,
    };
    use re_log_types::{StoreId, TimePoint, Timeline};
    use re_types::{ChunkId, archetypes, datatypes};

    #[derive(Debug, Clone, Copy)]
    enum StaticTestFlavor {
        /// First log a static chunk and then a regular chunk.
        StaticThenRegular { update_inbetween: bool },

        /// First log a regular chunk and then a static chunk.
        RegularThenStatic { update_inbetween: bool },

        /// Test case where we first log a static chunk and regular chunk and then later swap out the static chunk.
        /// This tests that we're able to invalidate the cache on static changes after the fact.
        PriorStaticThenRegularThenStatic { update_inbetween: bool },
    }

    const ALL_STATIC_TEST_FLAVOURS: [StaticTestFlavor; 6] = [
        StaticTestFlavor::StaticThenRegular {
            update_inbetween: true,
        },
        StaticTestFlavor::RegularThenStatic {
            update_inbetween: true,
        },
        StaticTestFlavor::PriorStaticThenRegularThenStatic {
            update_inbetween: true,
        },
        StaticTestFlavor::StaticThenRegular {
            update_inbetween: false,
        },
        StaticTestFlavor::RegularThenStatic {
            update_inbetween: false,
        },
        StaticTestFlavor::PriorStaticThenRegularThenStatic {
            update_inbetween: false,
        },
    ];

    #[derive(Default)]
    pub struct TestStoreSubscriber {
        unprocessed_events: Vec<ChunkStoreEvent>,
    }

    impl TestStoreSubscriber {
        /// Accesses the global store subscriber.
        ///
        /// Lazily registers the subscriber if it hasn't been registered yet.
        pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
            static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
            *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
        }

        /// Retrieves all transform events that have not been processed yet since the last call to this function.
        pub fn take_transform_events(store_id: &StoreId) -> Vec<ChunkStoreEvent> {
            ChunkStore::with_per_store_subscriber_mut(
                Self::subscription_handle(),
                store_id,
                |subscriber: &mut Self| std::mem::take(&mut subscriber.unprocessed_events),
            )
            .unwrap_or_default()
        }
    }

    impl PerStoreChunkSubscriber for TestStoreSubscriber {
        fn name() -> String {
            "TestStoreSubscriber".to_owned()
        }

        fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
            self.unprocessed_events.extend(events.cloned());
        }
    }

    fn apply_store_subscriber_events(cache: &mut TransformResolutionCache, entity_db: &EntityDb) {
        let events = TestStoreSubscriber::take_transform_events(entity_db.store_id());
        cache.process_store_events(events.iter());
    }

    fn static_test_setup_store(
        cache: &mut TransformResolutionCache,
        prior_static_chunk: Chunk,
        final_static_chunk: Chunk,
        regular_chunk: Chunk,
        flavor: StaticTestFlavor,
    ) -> Result<EntityDb, Box<dyn std::error::Error>> {
        // Print the flavor to its shown on test failure.
        println!("{flavor:?}");

        let mut entity_db = new_entity_db_with_subscriber_registered();

        match flavor {
            StaticTestFlavor::StaticThenRegular { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
            }

            StaticTestFlavor::RegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
            }

            StaticTestFlavor::PriorStaticThenRegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(prior_static_chunk))?;
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
            }
        }

        Ok(entity_db)
    }

    fn new_entity_db_with_subscriber_registered() -> EntityDb {
        let entity_db = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));
        let _ = TestStoreSubscriber::subscription_handle();
        entity_db
    }

    #[test]
    fn test_transforms_per_timeline_access() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk0 = Chunk::builder(EntityPath::from("with_transform"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()?;
        let chunk1 = Chunk::builder(EntityPath::from("without_transform"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                // Anything that doesn't have components the transform cache is interested in.
                &archetypes::Points3D::new([[1.0, 2.0, 3.0]]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk0))?;
        entity_db.add_chunk(&Arc::new(chunk1))?;

        apply_store_subscriber_events(&mut cache, &entity_db);
        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        assert!(
            transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "without_transform"
                )))
                .is_none()
        );
        assert!(
            transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "rando"
                )))
                .is_none()
        );
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "with_transform",
            )))
            .unwrap();
        #[cfg(debug_assertions)]
        assert_eq!(transforms.timeline, Some(*timeline.name()));
        assert_eq!(transforms.frame_transforms.len(), 1);
        assert_eq!(transforms.pose_transforms, None);
        assert_eq!(transforms.pinhole_projections, None);

        Ok(())
    }

    #[test]
    fn test_static_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &archetypes::Transform3D::update_fields()
                        .with_translation([123.0, 234.0, 345.0]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &archetypes::Transform3D::update_fields().with_translation([1.0, 2.0, 3.0]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Transform3D::update_fields().with_scale([123.0, 234.0, 345.0]),
                )
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                Some(SourceToTargetTransform {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                })
            );
            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
            );
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(SourceToTargetTransform {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(123.0, 234.0, 345.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(1.0, 2.0, 3.0),
                    ),
                })
            );

            // Timelines that the cache has never seen should still have the static transform.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();
            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(TimelineName::new("other"), 123)
                ),
                Some(SourceToTargetTransform {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_pose_transforms() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &archetypes::InstancePoses3D::new().with_translations([[321.0, 234.0, 345.0]]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &archetypes::InstancePoses3D::new()
                        .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    // Add a splatted scale.
                    &archetypes::InstancePoses3D::new().with_scales([[10.0, 20.0, 30.0]]),
                )
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            assert_eq!(
                transforms.latest_at_instance_poses(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                Some(&PoseTransformArchetypeMap {
                    instance_from_archetype_poses_per_archetype: IntMap::default(),
                    instance_from_poses: vec![
                        Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                        Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    ],
                })
            );
            assert_eq!(
                transforms
                    .latest_at_instance_poses(
                        &entity_db,
                        &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                    )
                    .cloned(),
                transforms
                    .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 0))
                    .cloned(),
            );
            assert_eq!(
                transforms
                    .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 1))
                    .map(|poses| &poses.instance_from_poses),
                Some(&vec![
                    Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(10.0, 20.0, 30.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(1.0, 2.0, 3.0),
                    ),
                    Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(10.0, 20.0, 30.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(4.0, 5.0, 6.0),
                    ),
                ])
            );

            // Timelines that the cache has never seen should still have the static poses.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();
            assert_eq!(
                transforms
                    .latest_at_instance_poses(
                        &entity_db,
                        &LatestAtQuery::new(TimelineName::new("other"), 123)
                    )
                    .map(|poses| &poses.instance_from_poses),
                Some(&vec![
                    Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                ])
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_pinhole_projection() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera_prior =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [123.0, 123.0],
                    [123.0, 123.0],
                );
            let image_from_camera_final =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [1.0, 2.0],
                    [1.0, 2.0],
                );

            // Static pinhole, non-static view coordinates.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &archetypes::Pinhole::new(image_from_camera_prior).with_resolution([1.0, 1.0]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &archetypes::Pinhole::new(image_from_camera_final).with_resolution([2.0, 2.0]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row([(timeline, 1)], &archetypes::ViewCoordinates::BLU())
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                Some(&ResolvedPinholeProjection {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
            assert_eq!(
                transforms
                    .latest_at_pinhole(
                        &entity_db,
                        &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                    )
                    .cloned(),
                transforms
                    .latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0))
                    .cloned(),
            );
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(&ResolvedPinholeProjection {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: components::ViewCoordinates::BLU,
                })
            );

            // Timelines that the cache has never seen should still have the static pinhole.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(TimelineName::new("other"), 123)
                ),
                Some(&ResolvedPinholeProjection {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_view_coordinates_projection() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [1.0, 2.0],
                    [1.0, 2.0],
                );

            // Static view coordinates, non-static pinhole.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BRU())
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BLU())
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Pinhole::new(image_from_camera),
                )
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            // There's view coordinates, but that doesn't show up.
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                None
            );
            assert_eq!(
                transforms
                    .latest_at_pinhole(
                        &entity_db,
                        &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                    )
                    .cloned(),
                transforms
                    .latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0))
                    .cloned(),
            );
            // Once we get a pinhole camera, the view coordinates should be there.
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(&ResolvedPinholeProjection {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera,
                    resolution: None,
                    view_coordinates: components::ViewCoordinates::BLU,
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                &archetypes::Transform3D::update_fields().with_scale([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 4)],
                &archetypes::Transform3D::from_rotation(glam::Quat::from_rotation_x(1.0)),
            )
            .with_archetype_auto_row([(timeline, 5)], &archetypes::Transform3D::clear_fields())
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline_name = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            None
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 2)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(1.0, 2.0, 3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                ),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 4)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_quat(glam::Quat::from_rotation_x(1.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 5)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::IDENTITY, // Empty transform is treated as connected with identity.
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 123)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::IDENTITY, // Empty transform is treated as connected with identity.
            })
        );

        Ok(())
    }

    #[test]
    fn test_pose_transforms_instance_poses_only() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::InstancePoses3D::new().with_translations([
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                ]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                // Less instances, and a splatted scale.
                &archetypes::InstancePoses3D::new()
                    .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
                    .with_scales([[2.0, 3.0, 4.0]]),
            )
            .with_archetype_auto_row(
                [(timeline, 4)],
                &archetypes::InstancePoses3D::clear_fields(),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 0)),
            None,
        );
        assert_eq!(
            transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 1))
                .map(|poses| &poses.instance_from_poses),
            Some(&vec![
                Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
            ])
        );
        assert_eq!(
            transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 2))
                .map(|poses| &poses.instance_from_poses),
            Some(&vec![
                Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
            ])
        );
        assert_eq!(
            transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 3))
                .map(|poses| &poses.instance_from_poses),
            Some(&vec![
                Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(2.0, 3.0, 4.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                ),
                Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(2.0, 3.0, 4.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(4.0, 5.0, 6.0),
                ),
            ])
        );

        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 4)),
            Some(&PoseTransformArchetypeMap::default())
        );
        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 123)),
            Some(&PoseTransformArchetypeMap::default())
        );

        Ok(())
    }

    #[test]
    fn test_mixing_instance_poses() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::InstancePoses3D::new().with_translations([
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                ]),
            )
            .with_archetype_auto_row(
                [(timeline, 2)],
                // Add some "base offset", but only for the first two items.
                &archetypes::Boxes3D::update_fields()
                    .with_centers([[10.0, 0.0, 0.0], [0.0, 100.0, 0.0]]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                // Rotate the box by 90 degrees around the Y axis.
                &archetypes::Boxes3D::update_fields().with_rotation_axis_angles([
                    datatypes::RotationAxisAngle::new(
                        glam::Vec3::new(0.0, 1.0, 0.0),
                        90.0_f32.to_radians(),
                    ),
                ]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // Pose for instances poses and non-boxes are unchanged over time.
        for t in 1..=4 {
            let instance_poses = transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, t))
                .unwrap();

            for archetype in [
                archetypes::InstancePoses3D::name(),
                "made_up_archetype".into(),
            ] {
                assert_eq!(
                    instance_poses.get(archetype),
                    [
                        Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                        Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                        Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                    ]
                );
            }
        }

        // Poses for boxes change over time.
        // T1
        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 1)),
            // All from `InstancePoses3D`
            Some(&PoseTransformArchetypeMap {
                instance_from_archetype_poses_per_archetype: IntMap::default(),
                instance_from_poses: vec![
                    Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            })
        );

        // T2
        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 2)),
            Some(&PoseTransformArchetypeMap {
                // All from `InstancePoses3D` combined with box centers.
                instance_from_archetype_poses_per_archetype: IntMap::from_iter([(
                    archetypes::Boxes3D::name(),
                    SmallVec1::try_from_slice(&[
                        Affine3A::from_translation(glam::Vec3::new(11.0, 2.0, 3.0)),
                        Affine3A::from_translation(glam::Vec3::new(4.0, 105.0, 6.0)),
                        Affine3A::from_translation(glam::Vec3::new(7.0, 108.0, 9.0)), // Affected by the last box center which is still splatted.
                    ])?
                )]),
                instance_from_poses: vec![
                    Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            })
        );

        // T3.
        let query_result = transforms
            .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, 3))
            .unwrap()
            .instance_from_archetype_poses_per_archetype
            .get(&archetypes::Boxes3D::name())
            .expect("Boxes3D archetype should be present");

        // More readable sanity check on translations which aren't affected by the rotation.
        assert_eq!(
            query_result[0].translation,
            glam::Vec3A::new(11.0, 2.0, 3.0)
        );
        // Since rotation isn't 100% accurate, we need to check for equality with a small tolerance.
        let eps = 0.000001;
        // Rotation on the first box affects all instances since it's splatted.
        let rotation =
            Affine3A::from_axis_angle(glam::Vec3::new(0.0, 1.0, 0.0), 90.0_f32.to_radians());
        let expected = Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)) * // Pose
            Affine3A::from_translation(glam::Vec3::new(10.0, 0.0, 0.0)) * rotation; // Box
        assert!(
            query_result[0].abs_diff_eq(expected, eps),
            "Expected: {:?}\nGot: {:?}",
            expected,
            query_result[0]
        );
        let expected = Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)) * // Pose
            (Affine3A::from_translation(glam::Vec3::new(0.0, 100.0, 0.0)) * rotation); // Box
        assert!(
            query_result[1].abs_diff_eq(expected, eps),
            "Expected: {:?}\nGot: {:?}",
            expected,
            query_result[1]
        );
        let expected = Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)) * // Pose
            (Affine3A::from_translation(glam::Vec3::new(0.0, 100.0, 0.0)) * rotation); // Box
        assert!(
            query_result[2].abs_diff_eq(expected, eps),
            "Expected: {:?}\nGot: {:?}",
            expected,
            query_result[2]
        );

        Ok(())
    }

    #[test]
    fn test_pinhole_projections() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let image_from_camera =
            components::PinholeProjection::from_focal_length_and_principal_point(
                [1.0, 2.0],
                [1.0, 2.0],
            );

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Pinhole::new(image_from_camera),
            )
            .with_archetype_auto_row([(timeline, 3)], &archetypes::ViewCoordinates::BLU())
            // Clear out the pinhole projection (this should yield nothing then for the remaining view coordinates.)
            .with_archetype_auto_row([(timeline, 4)], &archetypes::Pinhole::clear_fields())
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 0)),
            None
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 1)),
            Some(&ResolvedPinholeProjection {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera,
                resolution: None,
                view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
            })
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 2)),
            Some(&ResolvedPinholeProjection {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera,
                resolution: None,
                view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
            })
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 3)),
            Some(&ResolvedPinholeProjection {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera,
                resolution: None,
                view_coordinates: components::ViewCoordinates::BLU,
            })
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 4)),
            None // View coordinates alone doesn't give us a pinhole projection from the transform cache.
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, 123)),
            None
        );

        Ok(())
    }

    #[test]
    fn test_out_of_order_updates() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                // Note that this doesn't clear anything that could be inserted at time 2.
                &archetypes::Transform3D::update_fields().with_translation([2.0, 3.0, 4.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // Check that the transform cache has the expected transforms.
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 3)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(2.0, 3.0, 4.0)),
            })
        );

        // Add a transform between the two that invalidates the one at time stamp 3.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 2)],
                &archetypes::Transform3D::update_fields().with_scale([-1.0, -2.0, -3.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected changed transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // Check that the transform cache has the expected transforms.
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 2)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(-1.0, -2.0, -3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                ),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 3)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(-1.0, -2.0, -3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            })
        );

        Ok(())
    }

    #[test]
    fn test_clear_non_recursive() -> Result<(), Box<dyn std::error::Error>> {
        for clear_in_separate_chunk in [false, true] {
            println!("clear_in_separate_chunk: {clear_in_separate_chunk}");

            let mut entity_db = new_entity_db_with_subscriber_registered();
            let mut cache = TransformResolutionCache::default();

            let timeline = Timeline::new_sequence("t");
            let timeline_name = *timeline.name();

            let path = EntityPath::from("ent");
            let mut chunk = Chunk::builder(path.clone())
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
                )
                .with_archetype_auto_row(
                    [(timeline, 3)],
                    &archetypes::Transform3D::from_translation([3.0, 4.0, 5.0]),
                );
            if !clear_in_separate_chunk {
                chunk = chunk.with_archetype(
                    RowId::new(),
                    [(timeline, 2)],
                    &archetypes::Clear::new(false),
                );
            }
            entity_db.add_chunk(&Arc::new(chunk.build()?))?;

            if clear_in_separate_chunk {
                // If we're putting the clear in a separate chunk, we can try warming the cache and see whether we get the right transforms.
                {
                    apply_store_subscriber_events(&mut cache, &entity_db);
                    let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                    let transforms = transforms_per_timeline
                        .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                        .unwrap();
                    assert_eq!(
                        transforms
                            .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                        Some(SourceToTargetTransform {
                            target: TransformFrameIdHash::entity_path_hierarchy_root(),
                            transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                        })
                    );
                    assert_eq!(
                        transforms
                            .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                        Some(SourceToTargetTransform {
                            target: TransformFrameIdHash::entity_path_hierarchy_root(),
                            transform: Affine3A::from_translation(glam::Vec3::new(3.0, 4.0, 5.0)),
                        })
                    );
                }

                // Now add a separate chunk with a clear.
                let chunk = Chunk::builder(path.clone())
                    .with_archetype(
                        RowId::new(),
                        [(timeline, 2)],
                        &archetypes::Clear::new(false),
                    )
                    .build()?;
                entity_db.add_chunk(&Arc::new(chunk))?;
            }

            // Check transforms AFTER we apply the clear.
            {
                apply_store_subscriber_events(&mut cache, &entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                let transforms = transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                    .unwrap();

                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                    Some(SourceToTargetTransform {
                        target: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    })
                );
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 2)),
                    None
                );
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                    Some(SourceToTargetTransform {
                        target: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: Affine3A::from_translation(glam::Vec3::new(3.0, 4.0, 5.0)),
                    })
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_clear_recursive() -> Result<(), Box<dyn std::error::Error>> {
        for (clear_in_separate_chunk, update_after_each_chunk) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            println!(
                "clear_in_separate_chunk: {clear_in_separate_chunk}, apply_after_each_chunk: {update_after_each_chunk}",
            );

            let mut entity_db = new_entity_db_with_subscriber_registered();
            let mut cache = TransformResolutionCache::default();

            let timeline = Timeline::new_sequence("t");

            let mut parent_chunk = Chunk::builder(EntityPath::from("parent"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
                );
            if !clear_in_separate_chunk {
                parent_chunk = parent_chunk.with_archetype(
                    RowId::new(),
                    [(timeline, 2)],
                    &archetypes::Clear::new(true),
                );
            }
            entity_db.add_chunk(&Arc::new(parent_chunk.build()?))?;
            if update_after_each_chunk {
                apply_store_subscriber_events(&mut cache, &entity_db);
            }

            let child_chunk = Chunk::builder(EntityPath::from("parent/child"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
                );
            entity_db.add_chunk(&Arc::new(child_chunk.build()?))?;
            if update_after_each_chunk {
                apply_store_subscriber_events(&mut cache, &entity_db);
            }

            if clear_in_separate_chunk {
                let chunk = Chunk::builder(EntityPath::from("parent"))
                    .with_archetype(RowId::new(), [(timeline, 2)], &archetypes::Clear::new(true))
                    .build()?;
                entity_db.add_chunk(&Arc::new(chunk))?;
                if update_after_each_chunk {
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }

            let timeline = *timeline.name();
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);

            for path in [EntityPath::from("parent"), EntityPath::from("parent/child")] {
                let transform = transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                    .unwrap();

                println!("checking for correct transforms for path: {path:?}");

                assert_eq!(
                    transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 1)),
                    Some(SourceToTargetTransform {
                        target: TransformFrameIdHash::from_entity_path(&path.parent().unwrap()),
                        transform: Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    })
                );
                assert_eq!(
                    transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 2)),
                    None
                );
            }
        }

        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    enum SourceTargetChangesOverTimeTestMode {
        SingleChunk,
        MultipleChunksInOrder,
        MultipleChunksReverseOrder,
    }

    fn test_single_source_and_target_over_time(
        mode: SourceTargetChangesOverTimeTestMode,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Transform3D::update_fields().with_translation([1.0, 0.0, 0.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 2)],
                &archetypes::Transform3D::update_fields()
                    .with_translation([2.0, 0.0, 0.0])
                    .with_source_frame("frame0"),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                &archetypes::Transform3D::update_fields()
                    .with_translation([3.0, 0.0, 0.0])
                    .with_target_frame("frame1"),
            )
            .with_archetype_auto_row(
                [(timeline, 4)],
                &archetypes::Transform3D::update_fields()
                    .with_translation([4.0, 0.0, 0.0])
                    .with_source_frame("frame2")
                    .with_target_frame("frame3"),
            )
            .build()?;

        match mode {
            SourceTargetChangesOverTimeTestMode::SingleChunk => {
                entity_db.add_chunk(&Arc::new(chunk))?;
                apply_store_subscriber_events(&mut cache, &entity_db);
            }
            SourceTargetChangesOverTimeTestMode::MultipleChunksInOrder => {
                for row_idx in 0..chunk.num_rows() {
                    entity_db.add_chunk(&Arc::new(
                        chunk.row_sliced(row_idx, 1).with_id(ChunkId::new()),
                    ))?;
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }
            SourceTargetChangesOverTimeTestMode::MultipleChunksReverseOrder => {
                for row_idx in (0..chunk.num_rows()).rev() {
                    entity_db.add_chunk(&Arc::new(
                        chunk.row_sliced(row_idx, 1).with_id(ChunkId::new()),
                    ))?;
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }
        }

        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // State of the implicit frame over time.
        let transforms_implicit_frame = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();
        // Nothing we add over time affects the implicit frame whose relationship is set at frame 1
        for t in [1, 2, 3, 4, 5] {
            assert_eq!(
                transforms_implicit_frame
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(SourceToTargetTransform {
                    target: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: Affine3A::from_translation(glam::Vec3::new(1.0, 0.0, 0.0)),
                }),
                "querying at t=={t}"
            );
        }

        // State of frame0 over time.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            None
        );
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 2)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(2.0, 0.0, 0.0)),
            })
        );
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::from_str("frame1"),
                transform: Affine3A::from_translation(glam::Vec3::new(3.0, 0.0, 0.0)),
            })
        );
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 4)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::from_str("frame1"),
                transform: Affine3A::from_translation(glam::Vec3::new(3.0, 0.0, 0.0)),
            })
        );

        // frame1 is never a source, only a target.
        assert_eq!(
            timeline_transforms.frame_transforms(TransformFrameIdHash::from_str("custom_frame1")),
            None
        );

        // State of frame2 over time.
        let transforms_frame2 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame2"))
            .unwrap();
        for t in [1, 2, 3] {
            assert_eq!(
                transforms_frame2
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                None
            );
        }
        for t in [4, 5] {
            assert_eq!(
                transforms_frame2
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(SourceToTargetTransform {
                    target: TransformFrameIdHash::from_str("frame3"),
                    transform: Affine3A::from_translation(glam::Vec3::new(4.0, 0.0, 0.0)),
                }),
                "querying at t=={t}"
            );
        }

        // frame3 is never a source, only a target.
        assert_eq!(
            timeline_transforms.frame_transforms(TransformFrameIdHash::from_str("custom_frame3")),
            None
        );

        Ok(())
    }

    #[test]
    fn test_single_source_and_target_over_time_single_chunk()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_source_and_target_over_time(SourceTargetChangesOverTimeTestMode::SingleChunk)
    }

    #[test]
    fn test_single_source_and_target_over_time_multiple_chunks_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_source_and_target_over_time(
            SourceTargetChangesOverTimeTestMode::MultipleChunksInOrder,
        )
    }

    #[test]
    fn test_single_source_and_target_over_time_multiple_chunks_reverse_order()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_source_and_target_over_time(
            SourceTargetChangesOverTimeTestMode::MultipleChunksReverseOrder,
        )
    }

    #[test]
    fn test_static_source_frames() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let temporal_entity_path = EntityPath::from("my_entity");
        let static_entity_path = EntityPath::from("my_static_entity");

        entity_db.add_chunk(&Arc::new(
            Chunk::builder(static_entity_path.clone())
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::update_fields()
                        .with_translation([1.0, 0.0, 0.0])
                        .with_source_frame("frame0"),
                )
                .build()?,
        ))?;
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(temporal_entity_path)
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &archetypes::Transform3D::update_fields()
                        .with_translation([2.0, 0.0, 0.0])
                        .with_source_frame("frame1"),
                )
                .build()?,
        ))?;
        apply_store_subscriber_events(&mut cache, &entity_db);

        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // Check frame0 only ever sees the static transform.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 0.0, 0.0)),
            })
        );
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 0.0, 0.0)),
            })
        );

        // Check frame1 only ever sees the temporal transform.
        let transforms_frame1 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame1"))
            .unwrap();
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            None
        );
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(2.0, 0.0, 0.0)),
            })
        );

        // Now we change the static chunk to also talk about frame1 (but don't change anything else on it)
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(static_entity_path)
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::update_fields().with_source_frame("frame1"),
                )
                .build()?,
        ))?;
        apply_store_subscriber_events(&mut cache, &entity_db);

        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // Check frame0 is now empty all the way.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            None
        );
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            None
        );

        // Check frame1 has now both the static and the temporal transform visible.
        let transforms_frame1 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame1"))
            .unwrap();
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(1.0, 0.0, 0.0)),
            })
        );
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(SourceToTargetTransform {
                target: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: Affine3A::from_translation(glam::Vec3::new(2.0, 0.0, 0.0)),
            })
        );

        Ok(())
    }

    // TODO(andreas): We're missing tests for more corner cases involving source frames and (recursive) clears.

    #[test]
    fn test_gc() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity0"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Apply some updates to the transform before GC pass.
        apply_store_subscriber_events(&mut cache, &entity_db);

        let chunk = Chunk::builder(EntityPath::from("my_entity1"))
            .with_archetype_auto_row(
                [(timeline, 2)],
                &archetypes::Transform3D::from_translation([4.0, 5.0, 6.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Don't apply updates for this chunk.

        entity_db.gc(&GarbageCollectionOptions::gc_everything());
        apply_store_subscriber_events(&mut cache, &entity_db);

        // TODO(andreas): Ensure source ranges get GC'ed as well.
        // assert_eq!(
        //     cache
        //         .transforms_for_timeline(*timeline.name())
        //         .per_entity_source_ranges
        //         .clone(),
        //     cache.static_timeline.per_entity_source_ranges
        // );
        assert_eq!(
            cache
                .transforms_for_timeline(*timeline.name())
                .per_source_frame_transforms
                .clone(),
            cache.static_timeline.per_source_frame_transforms
        );

        Ok(())
    }

    // Tests GCing a recursive clear.
    #[test]
    fn test_gc_recursive_clear() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_recursive_clear"))
            .with_archetype(RowId::new(), [(timeline, 1)], &archetypes::Clear::new(true))
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Apply some updates to the transform before GC pass.
        apply_store_subscriber_events(&mut cache, &entity_db);

        assert!(
            cache
                .transforms_for_timeline(*timeline.name())
                .recursive_clears
                .contains_key(&EntityPath::from("my_recursive_clear")),
        );

        entity_db.gc(&GarbageCollectionOptions::gc_everything());
        apply_store_subscriber_events(&mut cache, &entity_db);

        assert!(
            cache
                .transforms_for_timeline(*timeline.name())
                .recursive_clears
                .is_empty(),
        );

        Ok(())
    }
}
