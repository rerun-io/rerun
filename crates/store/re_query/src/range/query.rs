use std::sync::Arc;

use arrow2::array::Array;
use itertools::Itertools;
use parking_lot::RwLock;

use re_chunk::RowId;
use re_chunk_store::{ChunkStore, LatestAtQuery, RangeQuery, TimeInt};
use re_log_types::{EntityPath, ResolvedTimeRange};
use re_types_core::{ComponentName, SizeBytes};

use crate::{CacheKey, Caches, RangeComponentResults, RangeComponentResultsInner, RangeResults};

// ---

impl Caches {
    /// Queries for the given `component_names` using range semantics.
    ///
    /// See [`RangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range(
        &self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> RangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = RangeResults::new(query.clone());

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = if crate::cacheable(component_name) {
                Arc::clone(
                    self.range_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(RangeCache::new(key.clone())))),
                )
            } else {
                // If the component shouldn't be cached, simply instantiate a new cache for it.
                // It will be dropped when the user is done with it.
                Arc::new(RwLock::new(RangeCache::new(key.clone())))
            };

            let mut cache = cache.write();

            // TODO(#4810): Get rid of this once we have proper bucketing in place.
            //
            // Detects the case where the user loads a piece of data at the end of the time range, then a piece
            // at the beginning of the range, and finally a piece right in the middle.
            //
            // DATA = ###################################################
            //          |      |     |       |            \_____/
            //          \______/     |       |            query #1
            //          query #2     \_______/
            //                       query #3
            //
            // and coarsly invalidates the whole cache in that case, to avoid the kind of bugs
            // showcased in <https://github.com/rerun-io/rerun/issues/5686>.
            {
                let time_range = cache.per_data_time.read_recursive().pending_time_range();
                if let Some(time_range) = time_range {
                    {
                        let hole_start = time_range.max();
                        let hole_end =
                            TimeInt::new_temporal(query.range().min().as_i64().saturating_sub(1));
                        if hole_start < hole_end {
                            let query = &LatestAtQuery::new(query.timeline(), hole_end);
                            if let Some((data_time, _, _)) =
                                crate::latest_at(store, query, entity_path, component_name)
                            {
                                if data_time > hole_start {
                                    re_log::trace!(%entity_path, %component_name, "coarsely invalidated because of bridged queries");
                                    cache.pending_invalidation = Some(TimeInt::MIN);
                                }
                            }
                        }
                    }

                    {
                        let hole_start = query.range().max();
                        let hole_end =
                            TimeInt::new_temporal(time_range.min().as_i64().saturating_sub(1));
                        if hole_start < hole_end {
                            let query = &LatestAtQuery::new(query.timeline(), hole_end);
                            if let Some((data_time, _, _)) =
                                crate::latest_at(store, query, entity_path, component_name)
                            {
                                if data_time > hole_start {
                                    re_log::trace!(%entity_path, %component_name, "coarsely invalidated because of bridged queries");
                                    cache.pending_invalidation = Some(TimeInt::MIN);
                                }
                            }
                        }
                    }
                }
            }

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
    pub per_data_time: RangeComponentResults,

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
            per_data_time: RangeComponentResults::default(),
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
            if let Some(time_front) = per_data_time_indices.front().map(|(t, _)| *t) {
                data_time_min = TimeInt::min(data_time_min, time_front);
            }
            if let Some(time_back) = per_data_time_indices.back().map(|(t, _)| *t) {
                data_time_max = TimeInt::max(data_time_max, time_back);
            }
        }

        strings.push(format!(
            "{} ({})",
            cache_key
                .timeline
                .typ()
                .format_range_utc(ResolvedTimeRange::new(data_time_min, data_time_max)),
            re_format::format_bytes(per_data_time.total_size_bytes() as _),
        ));

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

/// Implements the complete end-to-end range logic:
/// * Find all applicable `Chunk`s
/// * Apply a range filter to all of them
/// * Concatenate all the results (will sort them later)
pub fn range<'a>(
    store: &'a ChunkStore,
    query: &'a RangeQuery,
    entity_path: &EntityPath,
    component_name: ComponentName,
) -> impl Iterator<Item = (TimeInt, RowId, Box<dyn Array>)> + 'a {
    store
        .range_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        .map(move |chunk| chunk.range(query, component_name))
        .filter(|chunk| !chunk.is_empty())
        .flat_map(move |chunk| {
            chunk
                .iter_rows(&query.timeline(), &component_name)
                .filter_map(|(data_time, row_id, array)| {
                    array.map(|array| (data_time, row_id, array))
                })
                .collect_vec()
        })
}

impl RangeCache {
    /// Queries cached range data for a single component.
    pub fn range(
        &mut self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> RangeComponentResults {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        let Self {
            cache_key: _,
            per_data_time,
            pending_invalidation: _,
        } = self;

        let mut per_data_time = per_data_time.write();

        let query_front = per_data_time.compute_front_query(query);
        if let Some(query_front) = query_front.as_ref() {
            re_tracing::profile_scope!("front");

            for (data_time, row_id, array) in range(store, query_front, entity_path, component_name)
            {
                per_data_time
                    .promises_front
                    .push(((data_time, row_id), array));
            }
            {
                re_tracing::profile_scope!("sort front");
                per_data_time
                    .promises_front
                    .sort_by_key(|(index, _)| *index);
            }
        }

        if let Some(query_back) = per_data_time.compute_back_query(query, query_front.as_ref()) {
            re_tracing::profile_scope!("back");

            for (data_time, row_id, array) in range(store, &query_back, entity_path, component_name)
                // If there's static data to be found, the front query will take care of it already.
                .filter(|(data_time, _, _)| !data_time.is_static())
            {
                per_data_time
                    .promises_back
                    .push(((data_time, row_id), array));
            }
            {
                re_tracing::profile_scope!("sort back");
                per_data_time.promises_back.sort_by_key(|(index, _)| *index);
            }
        }

        per_data_time.sanity_check();
        drop(per_data_time);

        self.per_data_time.clone_at(query.range())
    }

    pub fn handle_pending_invalidation(&mut self) {
        re_tracing::profile_function!();

        let Self {
            cache_key: _,
            per_data_time,
            pending_invalidation,
        } = self;

        let Some(pending_invalidation) = pending_invalidation.take() else {
            return;
        };

        // Invalidating data is tricky. Our results object may have been cloned and shared already.
        // We can't just invalidate the data in-place without guaranteeing the post-invalidation query
        // will return the same results as the pending pre-invalidation queries.
        let mut new_inner = (*per_data_time.read()).clone();
        new_inner.truncate_at_time(pending_invalidation);
        per_data_time.inner = Arc::new(RwLock::new(new_inner));
    }
}

// ---

impl RangeComponentResultsInner {
    /// How many _indices_ across this entire cache?
    #[inline]
    pub fn num_indices(&self) -> u64 {
        self.indices.len() as _
    }

    /// How many _instances_ across this entire cache?
    #[inline]
    pub fn num_instances(&self) -> u64 {
        self.cached_dense
            .as_ref()
            .map_or(0u64, |cached| cached.dyn_num_values() as _)
    }

    /// Given a `query`, returns N reduced queries that are sufficient to fill the missing data
    /// on both the front & back sides of the cache.
    #[inline]
    pub fn compute_queries(&self, query: &RangeQuery) -> impl Iterator<Item = RangeQuery> {
        let front = self.compute_front_query(query);
        let back = self.compute_back_query(query, front.as_ref());
        front.into_iter().chain(back)
    }

    /// Given a `query`, returns a reduced query that is sufficient to fill the missing data
    /// on the front side of the cache, or `None` if all the necessary data is already
    /// cached.
    pub fn compute_front_query(&self, query: &RangeQuery) -> Option<RangeQuery> {
        let mut reduced_query = query.clone();

        // If the cache contains static data, then there's no point in querying anything else since
        // static data overrides everything anyway.
        if self
            .indices
            .front()
            .map_or(false, |(data_time, _)| data_time.is_static())
        {
            return None;
        }

        // Otherwise, query for what's missing on the front-side of the cache, while making sure to
        // take pending promises into account!
        //
        // Keep in mind: it is not possible for the cache to contain only part of a given
        // timestamp. All entries for a given timestamp are loaded and invalidated atomically,
        // whether it's promises or already resolved entries.

        // We check the back promises too just because I'm feeling overly cautious.
        // See `Concurrency edge-case` section below.

        if let Some(time_range) = self.pending_time_range() {
            let time_range_min = time_range.min().as_i64().saturating_sub(1);
            reduced_query
                .range
                .set_max(i64::min(reduced_query.range.max().as_i64(), time_range_min));
        } else {
            // If nothing has been cached already, then we just want to query everything.
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
    pub fn compute_back_query(
        &self,
        query: &RangeQuery,
        query_front: Option<&RangeQuery>,
    ) -> Option<RangeQuery> {
        let mut reduced_query = query.clone();

        // If the cache contains static data, then there's no point in querying anything else since
        // static data overrides everything anyway.
        if self
            .indices
            .front()
            .map_or(false, |(data_time, _)| data_time.is_static())
        {
            return None;
        }

        // Otherwise, query for what's missing on the back-side of the cache., while making sure to
        // take pending promises into account!
        //
        // Keep in mind: it is not possible for the cache to contain only part of a given
        // timestamp. All entries for a given timestamp are loaded and invalidated atomically,
        // whether it's promises or already resolved entries.

        if let Some(time_range) = self.pending_time_range() {
            let time_range_max = time_range.max().as_i64().saturating_add(1);
            reduced_query
                .range
                .set_min(i64::max(reduced_query.range.min().as_i64(), time_range_max));
        } else {
            // If nothing has been cached already, then the front query is already going to take care
            // of everything.
            return None;
        }

        // Back query should never overlap with the front query.
        // Reminder: time ranges are all inclusive.
        if let Some(query_front) = query_front {
            let front_max_plus_one = query_front.range().max().as_i64().saturating_add(1);
            let back_min = reduced_query.range().min().as_i64();
            reduced_query
                .range
                .set_min(i64::max(back_min, front_max_plus_one));
        }

        if reduced_query.range.max() < reduced_query.range.min() {
            return None;
        }

        Some(reduced_query)
    }
}
