use std::sync::Arc;

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
            cache.handle_pending_invalidation();
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
    // TODO(#4810): bucketize
    pub per_data_time: CachedRangeComponentResults,

    /// Everything greater than or equal to this timestamp has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching this criteria.
    /// `None` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidation: Option<TimeInt>,
}

impl RangeCache {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            per_data_time: CachedRangeComponentResults::default(),
            pending_invalidation: None,
        }
    }
}

impl std::fmt::Debug for RangeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            per_data_time,
            pending_invalidation: _,
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
            re_format::format_bytes(per_data_time.total_size_bytes() as _),
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
            cache_key,
            per_data_time,
            pending_invalidation,
        } = self;

        cache_key.heap_size_bytes()
            + per_data_time.heap_size_bytes()
            + pending_invalidation.heap_size_bytes()
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
            pending_invalidation: _,
        } = self;

        // A plain old `write()` (as opposed to a `try_write()`) here _should_ be fine.
        let mut per_data_time = per_data_time.write();

        if let Some(query_front) = per_data_time.compute_front_query(query) {
            eprintln!("front: {query_front:?}");
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
            eprintln!("back: {query_back:?}");
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

    pub fn handle_pending_invalidation(&mut self) {
        re_tracing::profile_function!();

        let Self {
            cache_key: _,
            per_data_time,
            pending_invalidation,
        } = self;

        let Some(pending_invalidation) = pending_invalidation else {
            return;
        };

        per_data_time
            .write()
            .truncate_at_time(*pending_invalidation);
    }
}

// ---

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

        // If nothing has been cached already, then we just want to query everything.
        if self.indices.is_empty() {
            return Some(reduced_query);
        }

        // If this cache contains static data, then there's no point in querying anything since
        // static data overrides everything else.
        if self
            .indices
            .front()
            .map_or(false, |(data_time, _)| data_time.is_static())
        {
            return None;
        }

        // Otherwise, query for what's missing on the front-side of the cache, while making sure to
        // take pending promises into account!

        let pending_front_min = self
            .promises_front
            .first()
            .map_or(TimeInt::MAX.as_i64(), |((t, _), _)| {
                t.as_i64().saturating_sub(1)
            });

        if let Some(time_range) = self.time_range() {
            let time_range_min = i64::min(
                time_range.min().as_i64().saturating_sub(1),
                pending_front_min,
            );
            reduced_query
                .range
                .set_max(i64::min(reduced_query.range.max().as_i64(), time_range_min));
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

        // If nothing has been cached already, then the front query is already going to take care
        // of everything.
        if self.indices.is_empty() {
            return None;
        }

        // If this cache contains static data, then there's no point in querying anything since
        // static data overrides everything else.
        if self
            .indices
            .front()
            .map_or(false, |(data_time, _)| data_time.is_static())
        {
            return None;
        }

        // Otherwise, query for what's missing on the front-side of the cache, while making sure to
        // take pending promises into account!

        let pending_back_max = self
            .promises_back
            .last()
            .map_or(TimeInt::MIN.as_i64(), |((t, _), _)| {
                t.as_i64().saturating_add(1)
            });

        if let Some(time_range) = self.time_range() {
            let time_range_max = i64::max(
                time_range.max().as_i64().saturating_add(1),
                pending_back_max,
            );
            reduced_query
                .range
                .set_min(i64::max(reduced_query.range.min().as_i64(), time_range_max));
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
