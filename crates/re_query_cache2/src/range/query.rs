use std::collections::BTreeSet;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;

use parking_lot::RwLock;
use re_data_store::{DataStore, RangeQuery, TimeInt};
use re_log_types::{EntityPath, TimeRange};
use re_query2::Promise;
use re_types_core::ComponentName;
use re_types_core::SizeBytes;

use crate::{
    CacheKey, CachedRangeComponentResults, CachedRangeComponentResultsInner, CachedRangeResults,
    Caches,
};

// ---

impl Caches {
    /// Queries for the given `component_names` using range semantics.
    ///
    /// See [`CachedRangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range(
        &self,
        store: &DataStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> CachedRangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = CachedRangeResults::default();

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);
            let cache = Arc::clone(
                self.range_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(RangeCache::new(key.clone())))),
            );

            let mut cache = cache.write();
            // cache.handle_pending_invalidation();
            let cached = cache.range(store, query, entity_path, component_name);
            results.add(component_name, cached);
        }

        results
    }
}

// ---

/// Caches the results of `Range` queries for a given [`CacheKey`].
pub struct RangeCache {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    /// All temporal data, organized by _data_ time.
    ///
    /// Query time is irrelevant for range queries.
    //
    // TODO(#4810): bucketize
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_data_time: CachedRangeComponentResults,

    /// These timestamps have been invalidated asynchronously.
    ///
    /// The next time this cache gets queried, it must remove any invalidated entries accordingly.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<TimeInt>,
}

impl RangeCache {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            per_data_time: CachedRangeComponentResults::default(),
            pending_invalidations: Default::default(),
        }
    }
}

impl std::fmt::Debug for RangeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let mut strings = Vec::new();

        let mut data_time_min = TimeInt::MAX;
        let mut data_time_max = TimeInt::MIN;

        {
            let per_data_time = per_data_time.read();

            let per_data_time_indices = &per_data_time.indices;
            if !per_data_time_indices.is_empty() {
                data_time_min = TimeInt::min(
                    data_time_min,
                    per_data_time_indices.front().map(|(t, _)| *t).unwrap(),
                );
                data_time_max = TimeInt::max(
                    data_time_max,
                    per_data_time_indices.back().map(|(t, _)| *t).unwrap(),
                );
            }
        }

        strings.push(format!(
            "{} ({})",
            cache_key
                .timeline
                .typ()
                .format_range_utc(TimeRange::new(data_time_min, data_time_max)),
            re_format::format_bytes((0) as _),
            // TODO
            // re_format::format_bytes((per_data_time.cached_heap_size_bytes.load(Relaxed)) as _),
        ));
        strings.push(indent::indent_all_by(2, format!("{per_data_time:?}")));

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for RangeCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key: _,
            per_data_time,
            pending_invalidations,
        } = self;

        // TODO
        // let per_data_time = per_data_time.total_size_bytes();
        let per_data_time = 0;
        let pending_invalidations = pending_invalidations.total_size_bytes();

        per_data_time + pending_invalidations
    }
}

impl RangeCache {
    /// Queries cached range data for a single component.
    pub fn range(
        &mut self,
        store: &DataStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> CachedRangeComponentResults {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        let RangeCache {
            cache_key: _,
            per_data_time,
            pending_invalidations: _,
        } = self;

        // TODO: front & back, and keep them sorted too

        // TODO: lock?
        let mut per_data_time = per_data_time.write();

        if let Some(query_front) = per_data_time.compute_front_query(query) {
            for (data_time, row_id, mut cells) in
                store.range(&query_front, entity_path, [component_name])
            {
                // Soundness:
                // * `cells[0]` is guaranteed to exist since we passed in `&[component_name]`
                // * `cells[0]` is guaranteed to be non-null, otherwise this whole result would be null
                let Some(cell) = cells[0].take() else {
                    debug_assert!(cells[0].is_some(), "unreachable: `cells[0]` is missing");
                    continue;
                };

                per_data_time
                    .promises_front
                    .push(((data_time, row_id), Promise::new(cell)));
                per_data_time
                    .promises_front
                    .sort_by_key(|(index, _)| *index);
            }
        }

        if let Some(query_back) = per_data_time.compute_back_query(query) {
            for (data_time, row_id, mut cells) in
                store.range(&query_back, entity_path, [component_name])
            {
                // Soundness:
                // * `cells[0]` is guaranteed to exist since we passed in `&[component_name]`
                // * `cells[0]` is guaranteed to be non-null, otherwise this whole result would be null
                let Some(cell) = cells[0].take() else {
                    debug_assert!(cells[0].is_some(), "unreachable: `cells[0]` is missing");
                    continue;
                };

                per_data_time
                    .promises_back
                    .push(((data_time, row_id), Promise::new(cell)));
                per_data_time.promises_back.sort_by_key(|(index, _)| *index);
            }
        }

        per_data_time.sanity_check();
        drop(per_data_time);

        self.per_data_time.clone()
    }

    #[cfg(target_os = "todo")]
    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            per_data_time,
            pending_invalidations,
        } = self;

        let pending_invalidations = std::mem::take(pending_invalidations);

        // First, remove any data indexed by a _query time_ that's more recent than the oldest
        // _data time_ that's been invalidated.
        //
        // Note that this data time might very well be `TimeInt::STATIC`, in which case the entire
        // query-time-based index will be dropped.
        if let Some(&oldest_data_time) = pending_invalidations.first() {
            per_query_time.retain(|&query_time, _| query_time < oldest_data_time);
        }

        // Second, remove any data indexed by _data time_, if it's been invalidated.
        per_data_time.retain(|data_time, _| !pending_invalidations.contains(data_time));
    }
}

// ---

// TODO: this shouldn't be declared on this object -- we cant be releasing the lock like that
impl CachedRangeComponentResultsInner {
    /// Given a `query`, returns N reduced queries that are sufficient to fill the missing data
    /// on both the front & back sides of the cache.
    #[inline]
    pub fn compute_queries(&self, query: &RangeQuery) -> impl Iterator<Item = RangeQuery> {
        let front = self.compute_front_query(query);
        let back = self.compute_back_query(query);
        front.into_iter().chain(back)
    }

    /// Given a `query`, returns a reduced query that is sufficient to fill the missing data
    /// on the front side of the cache, or `None` if all the necessary data is already
    /// cached.
    pub fn compute_front_query(&self, query: &RangeQuery) -> Option<RangeQuery> {
        let mut reduced_query = query.clone();

        if self.indices.is_empty() {
            return Some(reduced_query);
        }

        let pending_front_min = self
            .promises_front
            .first()
            .map_or(i64::MAX, |((t, _), _)| t.as_i64().saturating_sub(1));

        if let Some(bucket_time_range) = self.time_range() {
            let bucket_time_range_min = i64::min(
                bucket_time_range.min().as_i64().saturating_sub(1),
                pending_front_min,
            );
            reduced_query.range.set_max(i64::min(
                reduced_query.range.max().as_i64(),
                bucket_time_range_min,
            ));
        } else {
            reduced_query.range.set_max(i64::min(
                reduced_query.range.max().as_i64(),
                pending_front_min,
            ));
            return Some(reduced_query);
        }

        if reduced_query.range.max() < reduced_query.range.min() {
            return None;
        }

        Some(reduced_query)
    }

    /// Given a `query`, returns a reduced query that is sufficient to fill the missing data
    /// on the back side of the cache, or `None` if all the necessary data is already
    /// cached.
    pub fn compute_back_query(&self, query: &RangeQuery) -> Option<RangeQuery> {
        let mut reduced_query = query.clone();

        // TODO: explain
        if self.indices.is_empty() {
            return None;
        }

        let pending_back_max = self
            .promises_back
            .last()
            .map_or(i64::MIN, |((t, _), _)| t.as_i64().saturating_add(1));

        if let Some(bucket_time_range) = self.time_range() {
            let bucket_time_range_max = i64::max(
                bucket_time_range.max().as_i64().saturating_add(1),
                pending_back_max,
            );
            reduced_query.range.set_min(i64::max(
                reduced_query.range.min().as_i64(),
                bucket_time_range_max,
            ));
        } else {
            reduced_query.range.set_min(i64::max(
                reduced_query.range.min().as_i64(),
                pending_back_max,
            ));
            return Some(reduced_query);
        }

        if reduced_query.range.max() < reduced_query.range.min() {
            return None;
        }

        Some(reduced_query)
    }
}
