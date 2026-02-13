use std::collections::BTreeSet;

use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_byte_size::SizeBytes;
use re_chunk_store::{LatestAtQuery, MissingChunkReporter};
use re_entity_db::EntityDb;
use re_log::debug_assert;
use re_log_types::{EntityPath, TimeInt, TimelineName};

use crate::TransformFrameIdHash;
use crate::transform_queries::{
    query_and_resolve_pinhole_projection_at_entity, query_and_resolve_tree_transform_at_entity,
};

use super::cached_transform_value::{
    CachedTransformValue, add_invalidated_entry_if_not_already_cleared,
};
use super::cached_transforms_for_timeline::CachedTransformsForTimeline;
use super::parent_from_child_transform::ParentFromChildTransform;
use super::resolved_pinhole_projection::ResolvedPinholeProjection;
use super::transforms_for_child_frame_events::TransformsForChildFrameEvents;

/// Cached transforms from a single child frame to a (potentially changing) parent frame over time.
///
/// Incorporates any static transforms that may apply to this entity.
///
/// Time points are conservative: it can happen that we generate new events (==cache slots) despite no change
/// occurring for this child frame.
/// However, we mustn't ever note down timepoints at which the given child frame is not "active" on its entity.
/// Doing so would mean that queries using `re_query` yield information about a _different_ child frame
/// which we then can't add to the cache entries of the current frame.
#[derive(Debug)]
pub struct TreeTransformsForChildFrame {
    // Is None if this is about static time.
    #[cfg(debug_assertions)]
    pub timeline: Option<TimelineName>,

    /// The entity path that produces temporal information for this frame.
    ///
    /// Note that it is a user-data error to change the entity path a frame relationship is defined on.
    /// I.e., given a frame relationship `A -> B` logged on entity `/my_path`, all future changes
    /// to the relation of `A ->` must be logged on the same entity `/my_path`.
    ///
    /// This greatly simplifies clearing and tracking of transforms.
    pub associated_entity_path_temporal: Option<EntityPath>,

    /// Like [`Self::associated_entity_path_temporal`] but for static chunks.
    pub associated_entity_path_static: Option<EntityPath>,

    pub child_frame: TransformFrameIdHash,

    pub events: RwLock<TransformsForChildFrameEvents>,
}

impl Clone for TreeTransformsForChildFrame {
    fn clone(&self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: self.timeline,
            associated_entity_path_temporal: self.associated_entity_path_temporal.clone(),
            associated_entity_path_static: self.associated_entity_path_static.clone(),
            child_frame: self.child_frame,
            events: RwLock::new(self.events.read().clone()),
        }
    }
}

impl PartialEq for TreeTransformsForChildFrame {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            associated_entity_path_temporal,
            associated_entity_path_static,
            child_frame,
            events,
        } = self;

        associated_entity_path_temporal == &other.associated_entity_path_temporal
            && associated_entity_path_static == &other.associated_entity_path_static
            && child_frame == &other.child_frame
            && *events.read() == *other.events.read()
    }
}

impl SizeBytes for TreeTransformsForChildFrame {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            associated_entity_path_temporal,
            associated_entity_path_static,
            child_frame,
            events,

            #[cfg(debug_assertions)]
                timeline: _,
        } = self;

        associated_entity_path_temporal.heap_size_bytes()
            + associated_entity_path_static.heap_size_bytes()
            + child_frame.heap_size_bytes()
            + events.read().heap_size_bytes()
    }
}

impl TreeTransformsForChildFrame {
    pub fn new_temporal(
        associated_entity_path: EntityPath,
        child_frame: TransformFrameIdHash,
        _timeline: TimelineName,
        static_timeline: &CachedTransformsForTimeline,
        non_recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
        recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
    ) -> Self {
        let mut events = TransformsForChildFrameEvents::new_empty();

        // Take over static events.
        let associated_entity_path_static = if let Some(static_transforms) =
            static_timeline.per_child_frame_transforms.get(&child_frame)
        {
            events = static_transforms.events.read().clone();

            debug_assert!(static_transforms.associated_entity_path_static.is_some());
            static_transforms.associated_entity_path_static.clone()
        } else {
            None
        };

        // Take over clear events.
        if let Some(cleared_times) = non_recursive_clears.get(&associated_entity_path) {
            events.insert_clears(cleared_times);
        }
        for (recursively_cleared_path, times) in recursive_clears {
            if associated_entity_path.starts_with(recursively_cleared_path) {
                events.insert_clears(times);
            }
        }

        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            associated_entity_path_temporal: Some(associated_entity_path),
            associated_entity_path_static,
            child_frame,
            events: RwLock::new(events),
        }
    }

    pub fn new_for_new_empty_timeline(
        _timeline: TimelineName,
        static_timeline_entry: &Self,
    ) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            ..static_timeline_entry.clone()
        }
    }

    pub fn new_static(
        associated_entity_path: EntityPath,
        child_frame: TransformFrameIdHash,
    ) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: None,
            associated_entity_path_temporal: None,
            associated_entity_path_static: Some(associated_entity_path),
            child_frame,
            events: RwLock::new(TransformsForChildFrameEvents::new_empty()),
        }
    }

    /// The entity path that produces information for this frame.
    pub fn associated_entity_path(&self, time: TimeInt) -> &EntityPath {
        if time == TimeInt::STATIC {
            // Use static path if it exists.
            self.associated_entity_path_static
                .as_ref()
                .or(self.associated_entity_path_temporal.as_ref())
                .expect("Either temporal or static associated entity path must be set")
        } else {
            // Use temporal path if it exists.
            self.associated_entity_path_temporal
                .as_ref()
                .or(self.associated_entity_path_static.as_ref())
                .expect("Either temporal or static associated entity path must be set")
        }
    }

    /// Inserts an invalidation point for transforms.
    pub fn invalidate_transform_at(&mut self, time: TimeInt) {
        let events = self.events.get_mut();
        add_invalidated_entry_if_not_already_cleared(&mut events.frame_transforms, time);
    }

    /// Inserts an invalidation point for pinhole projections.
    pub fn invalidate_pinhole_projection_at(&mut self, time: TimeInt) {
        let events = self.events.get_mut();
        add_invalidated_entry_if_not_already_cleared(&mut events.pinhole_projections, time);
    }

    #[inline]
    pub fn latest_at_transform(
        &self,
        entity_db: &EntityDb,
        missing_chunk_reporter: &MissingChunkReporter,
        query: &LatestAtQuery,
    ) -> Option<ParentFromChildTransform> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let mut events = self.events.write();

        events
            .frame_transforms
            .mutate_latest_at(
                &query.at(),
                |time_of_last_update_to_this_frame, frame_transform| {
                    // Separate check to work around borrow checker issues.
                    if frame_transform == &CachedTransformValue::Invalidated {
                        let transform = query_and_resolve_tree_transform_at_entity(
                            entity_db,
                            missing_chunk_reporter,
                            self.associated_entity_path(*time_of_last_update_to_this_frame),
                            self.child_frame,
                            // Do NOT use the original query time since that may give us information about a different child frame!
                            &LatestAtQuery::new(
                                query.timeline(),
                                *time_of_last_update_to_this_frame,
                            ),
                        );

                        // First, we update the cache value.
                        *frame_transform = match &transform {
                            Ok(transform) => CachedTransformValue::Resident(transform.clone()),

                            Err(crate::transform_queries::TransformError::MissingTransform {
                                ..
                            }) => {
                                // This can happen if we conservatively added a timepoint before any transform event happened.
                                CachedTransformValue::Cleared
                            }

                            Err(err) => {
                                // Only warn since we can still work just fine if a transform didn't work.
                                re_log::warn_once!("Failed to query transformations: {err}");
                                CachedTransformValue::Cleared
                            }
                        };
                    }

                    match frame_transform {
                        CachedTransformValue::Resident(transform) => Some(transform.clone()),
                        CachedTransformValue::Cleared => None,
                        CachedTransformValue::Invalidated => {
                            unreachable!("Just made transform cache-resident")
                        }
                    }
                },
            )
            .flatten()
    }

    #[inline]
    pub fn latest_at_pinhole(
        &self,
        entity_db: &EntityDb,
        missing_chunk_reporter: &MissingChunkReporter,
        query: &LatestAtQuery,
    ) -> Option<ResolvedPinholeProjection> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let mut events = self.events.write();

        events
            .pinhole_projections
            .mutate_latest_at(
                &query.at(),
                |time_of_last_update_to_this_frame, pinhole_projection| {
                    // Separate check to work around borrow checker issues.
                    if pinhole_projection == &CachedTransformValue::Invalidated {
                        let transform = query_and_resolve_pinhole_projection_at_entity(
                            entity_db,
                            missing_chunk_reporter,
                            self.associated_entity_path(*time_of_last_update_to_this_frame),
                            self.child_frame,
                            // Do NOT use the original query time since that may give us information about a different child frame!
                            &LatestAtQuery::new(
                                query.timeline(),
                                *time_of_last_update_to_this_frame,
                            ),
                        );

                        *pinhole_projection = match &transform {
                            Ok(transform) => CachedTransformValue::Resident(transform.clone()),

                            Err(crate::transform_queries::TransformError::MissingTransform {
                                ..
                            }) => {
                                // This can happen if we conservatively added a timepoint before any transform event happened.
                                CachedTransformValue::Cleared
                            }

                            Err(err) => {
                                // Only warn since we can still work just fine if a transform didn't work.
                                re_log::warn_once!("Failed to query transformations: {err}");
                                CachedTransformValue::Cleared
                            }
                        };
                    }

                    match pinhole_projection {
                        CachedTransformValue::Resident(transform) => Some(transform.clone()),
                        CachedTransformValue::Cleared => None,
                        CachedTransformValue::Invalidated => {
                            unreachable!("Just made transform cache-resident")
                        }
                    }
                },
            )
            .flatten()
    }
}
