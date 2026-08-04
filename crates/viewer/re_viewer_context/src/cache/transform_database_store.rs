use std::sync::Arc;

use ahash::{HashMap, HashSet};
use parking_lot::{ArcRwLockReadGuard, RawRwLock};
use re_byte_size::SizeBytes as _;
use re_chunk::{LatestAtQuery, TimelineName};
use re_chunk_store::{ChunkStoreEvent, MissingChunkReporter};
use re_entity_db::EntityDb;
use re_tf::{
    CachedTransformsForTimeline, FrameIdRegistry, TransformForest, TransformResolutionCache,
};

use super::Cache;

/// Stores a [`TransformResolutionCache`] for each recording.
///
/// Ensures that the cache stays up to date.
#[derive(Default, re_byte_size::SizeBytes)]
#[size_bytes(profile)]
pub struct TransformDatabaseStoreCache {
    transform_cache: Option<TransformResolutionCache>,

    /// Query for the forest exposed by [`Self::transform_forest`].
    transform_forest_query: Option<LatestAtQuery>,

    /// Transform forest snapshots keyed by the time query used to build them.
    ///
    /// Data-time transform resolution can request many entities at the same acquisition timestamp.
    /// Keeping the entire forest avoids repeatedly walking the transform cache for each entity.
    transform_forests: HashMap<LatestAtQuery, Arc<re_tf::TransformForest>>,

    /// Forest queries used in the current frame.
    ///
    /// At the next memory purge, snapshots that weren't reused are evicted. This bounds memory
    /// while preserving forests for stable data timestamps across consecutive frames.
    used_transform_queries: HashSet<LatestAtQuery>,

    /// Timelines that were used in the current frame.
    /// Used for evicting unused timelines at the beginning of the next frame.
    used_timelines: HashSet<TimelineName>,
}

impl TransformDatabaseStoreCache {
    /// Returns the registry of all known frame ids.
    #[inline]
    pub fn frame_id_registry(
        &mut self,
        entity_db: &EntityDb,
    ) -> ArcRwLockReadGuard<RawRwLock, FrameIdRegistry> {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        transform_cache.frame_id_registry()
    }

    /// Returns the registry of all known frames if it has already been initialized, or `None` if it hasn't.
    #[inline]
    pub fn cached_frame_id_registry(
        &self,
    ) -> Option<ArcRwLockReadGuard<RawRwLock, FrameIdRegistry>> {
        self.transform_cache
            .as_ref()
            .map(TransformResolutionCache::frame_id_registry)
    }

    /// Accesses the transform component tracking data for a given timeline.
    #[inline]
    pub fn transforms_for_timeline(
        &mut self,
        entity_db: &EntityDb,
        timeline: TimelineName,
    ) -> ArcRwLockReadGuard<RawRwLock, CachedTransformsForTimeline> {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        // Remember that this timeline was used this frame.
        self.used_timelines.insert(timeline);

        transform_cache
            .ensure_timeline_is_initialized(entity_db.storage_engine().store(), timeline);

        transform_cache.transforms_for_timeline(timeline)
    }

    /// Returns a snapshot of the transform cache for a single latest-at time.
    ///
    /// The snapshot contains registered frames matching the frame filter plus latest direct
    /// transform edges between them.
    pub fn latest_at_transform_cache_snapshot(
        &mut self,
        entity_db: &EntityDb,
        missing_chunk_reporter: &MissingChunkReporter,
        query: &LatestAtQuery,
        filter: re_tf::transform_cache_snapshot::SnapshotFilter,
    ) -> re_tf::transform_cache_snapshot::Snapshot {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        if let Some(timeline) = query.timeline() {
            // Remember that this timeline was used this frame.
            self.used_timelines.insert(timeline);

            transform_cache
                .ensure_timeline_is_initialized(entity_db.storage_engine().store(), timeline);
        }

        let frame_id_registry = transform_cache.frame_id_registry();
        let transforms = transform_cache.transforms_for_timeline(query.timeline());
        transforms.latest_at_transform_cache_snapshot(
            &frame_id_registry,
            entity_db,
            missing_chunk_reporter,
            query,
            filter,
        )
    }

    pub fn update_transform_forest(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Arc<re_tf::TransformForest> {
        re_tracing::profile_function!();

        let transform_forest = self.transform_forest_for_query(entity_db, query);
        self.transform_forest_query = Some(query.clone());
        transform_forest
    }

    /// Returns the cached transform forest for `query`, building it on first use.
    pub fn transform_forest_for_query(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Arc<re_tf::TransformForest> {
        re_tracing::profile_function!();

        if let Some(timeline) = query.timeline() {
            self.used_timelines.insert(timeline);
        }
        self.used_transform_queries.insert(query.clone());

        if let Some(transform_forest) = self.transform_forests.get(query) {
            return transform_forest.clone();
        }

        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        if let Some(timeline) = query.timeline() {
            transform_cache
                .ensure_timeline_is_initialized(entity_db.storage_engine().store(), timeline);
        }

        let transform_forest = Arc::new(TransformForest::new(entity_db, transform_cache, query));
        self.transform_forests
            .insert(query.clone(), transform_forest.clone());
        transform_forest
    }

    pub fn transform_forest(&self) -> Option<Arc<re_tf::TransformForest>> {
        self.transform_forest_query
            .as_ref()
            .and_then(|query| self.transform_forests.get(query))
            .cloned()
    }
}

impl Cache for TransformDatabaseStoreCache {
    fn name(&self) -> &'static str {
        "TransformDatabaseStoreCache"
    }

    fn begin_frame(&mut self) {
        self.used_timelines.clear();
        self.used_transform_queries.clear();
    }

    fn purge_memory(&mut self) {
        self.transform_forests
            .retain(|query, _| self.used_transform_queries.contains(query));

        if let Some(transform_cache) = &mut self.transform_cache {
            // Evict all timelines that weren't used in the last frame.
            // They will be lazily re-initialized if needed again.
            let unused_timelines = transform_cache
                .cached_timelines()
                .filter(|t| !self.used_timelines.contains(t))
                .collect::<Vec<_>>();

            for timeline in unused_timelines {
                transform_cache.evict_timeline_cache(timeline);
            }
        }
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        if let Some(transform_cache) = &mut self.transform_cache {
            transform_cache.process_store_events(events.iter().copied());
        }

        // A transform row may have been inserted in the past or removed, changing the latest-at
        // result for any cached query. Rebuild snapshots lazily after every store mutation.
        self.transform_forests.clear();
        self.transform_forest_query = None;
    }
}

impl re_byte_size::MemUsageTreeCapture for TransformDatabaseStoreCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            used_timelines,
            used_transform_queries,
            transform_cache,
            transform_forest_query,
            transform_forests,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();

        node.add("used_timelines", used_timelines.total_size_bytes());
        node.add(
            "used_transform_queries",
            used_transform_queries.total_size_bytes(),
        );
        node.add("transform_cache", transform_cache.capture_mem_usage_tree());
        node.add(
            "transform_forest_query",
            transform_forest_query.total_size_bytes(),
        );
        node.add("transform_forests", transform_forests.total_size_bytes());

        node.into_tree()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{LatestAtQuery, TimelineName};
    use re_entity_db::EntityDb;
    use re_log_types::StoreInfo;

    use super::{Cache as _, TransformDatabaseStoreCache};

    #[test]
    fn transform_forests_are_cached_per_latest_at_query() {
        let entity_db = EntityDb::new(StoreInfo::testing().store_id);
        let mut cache = TransformDatabaseStoreCache::default();
        let timeline = TimelineName::from("frame");
        let query_at_one = LatestAtQuery::new(timeline, 1);
        let query_at_two = LatestAtQuery::new(timeline, 2);

        let first = cache.transform_forest_for_query(&entity_db, &query_at_one);
        let same_query = cache.transform_forest_for_query(&entity_db, &query_at_one);
        let other_query = cache.transform_forest_for_query(&entity_db, &query_at_two);

        assert!(Arc::ptr_eq(&first, &same_query));
        assert!(!Arc::ptr_eq(&first, &other_query));

        cache.begin_frame();
        let first_in_next_frame = cache.transform_forest_for_query(&entity_db, &query_at_one);
        cache.purge_memory();

        let first_after_purge = cache.transform_forest_for_query(&entity_db, &query_at_one);
        let other_after_purge = cache.transform_forest_for_query(&entity_db, &query_at_two);
        assert!(Arc::ptr_eq(&first, &first_in_next_frame));
        assert!(Arc::ptr_eq(&first, &first_after_purge));
        assert!(!Arc::ptr_eq(&other_query, &other_after_purge));
    }
}
