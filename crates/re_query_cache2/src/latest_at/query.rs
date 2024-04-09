use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::Arc};

use indexmap::IndexMap;
use itertools::Itertools;
use parking_lot::RwLock;

use re_data_store::{DataStore, LatestAtQuery, TimeInt};
use re_log_types::EntityPath;
use re_query2::Promise;
use re_types_core::ComponentName;
use re_types_core::SizeBytes;

use crate::{CacheKey, CachedLatestAtComponentResults, CachedLatestAtResults, Caches};

// ---

impl Caches {
    /// Queries for the given `component_names` using latest-at semantics.
    ///
    /// See [`CachedLatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at(
        &self,
        store: &DataStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> CachedLatestAtResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = CachedLatestAtResults::default();

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);
            let cache = Arc::clone(
                self.latest_at_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
            );

            let mut cache = cache.write();
            cache.handle_pending_invalidation();
            if let Some(cached) = cache.latest_at(store, query, entity_path, component_name) {
                results.add(component_name, cached);
            }
        }

        results
    }
}

// ---

/// Caches the results of `LatestAt` queries for a given [`CacheKey`].
pub struct LatestAtCache {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query and check
    /// if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
    //
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_query_time: BTreeMap<TimeInt, Arc<CachedLatestAtComponentResults>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    //
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_data_time: BTreeMap<TimeInt, Arc<CachedLatestAtComponentResults>>,

    /// These timestamps have been invalidated asynchronously.
    ///
    /// The next time this cache gets queried, it must remove any invalidated entries accordingly.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<TimeInt>,
}

impl LatestAtCache {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            per_query_time: Default::default(),
            per_data_time: Default::default(),
            pending_invalidations: Default::default(),
        }
    }
}

impl std::fmt::Debug for LatestAtCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            per_query_time,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let mut strings = Vec::new();

        struct StatsPerBucket {
            query_times: BTreeSet<TimeInt>,
            data_time: TimeInt,
            total_size_bytes: u64,
        }

        let mut buckets: IndexMap<_, _> = per_data_time
            .iter()
            .map(|(&data_time, bucket)| {
                (
                    Arc::as_ptr(bucket),
                    StatsPerBucket {
                        query_times: Default::default(),
                        data_time,
                        total_size_bytes: bucket.total_size_bytes(),
                    },
                )
            })
            .collect();

        for (&query_time, bucket) in per_query_time {
            if let Some(bucket) = buckets.get_mut(&Arc::as_ptr(bucket)) {
                bucket.query_times.insert(query_time);
            }
        }

        for bucket in buckets.values() {
            strings.push(format!(
                "query_times=[{}] -> data_time={:?} ({})",
                bucket
                    .query_times
                    .iter()
                    .map(|t| cache_key.timeline.typ().format_utc(*t))
                    .collect_vec()
                    .join(", "),
                bucket.data_time.as_i64(),
                re_format::format_bytes(bucket.total_size_bytes as _),
            ));
        }

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for LatestAtCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations,
        } = self;

        let per_query_time = per_query_time
            .keys()
            .map(|k| k.total_size_bytes())
            .sum::<u64>();
        let per_data_time = per_data_time.total_size_bytes();
        let pending_invalidations = pending_invalidations.total_size_bytes();

        per_query_time + per_data_time + pending_invalidations
    }
}

impl LatestAtCache {
    /// Queries cached latest-at data for a single component.
    pub fn latest_at(
        &mut self,
        store: &DataStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Option<Arc<CachedLatestAtComponentResults>> {
        re_tracing::profile_scope!("latest_at", format!("{query:?}"));

        let LatestAtCache {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let query_time_bucket_at_query_time = match per_query_time.entry(query.at()) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                // Fastest path: we have an entry for this exact query time, no need to look any
                // further.
                re_log::trace!(query_time=?query.at(), "cache hit (query time)");
                return Some(Arc::clone(entry.get()));
            }
            std::collections::btree_map::Entry::Vacant(entry) => entry,
        };

        let result = store.latest_at(query, entity_path, component_name, &[component_name]);

        // NOTE: cannot `result.and_then(...)` or borrowck gets lost.
        if let Some((data_time, row_id, mut cells)) = result {
            // Fast path: we've run the query and realized that we already have the data for the resulting
            // _data_ time, so let's use that to avoid join & deserialization costs.
            if let Some(data_time_bucket_at_data_time) = per_data_time.get(&data_time) {
                re_log::trace!(query_time=?query.at(), ?data_time, "cache hit (data time)");

                query_time_bucket_at_query_time.insert(Arc::clone(data_time_bucket_at_data_time));

                // We now know for a fact that a query at that data time would yield the same
                // results: copy the bucket accordingly so that the next cache hit for that query
                // time ends up taking the fastest path.
                let query_time_bucket_at_data_time = per_query_time.entry(data_time);
                query_time_bucket_at_data_time
                    .and_modify(|v| *v = Arc::clone(data_time_bucket_at_data_time))
                    .or_insert(Arc::clone(data_time_bucket_at_data_time));

                return Some(Arc::clone(data_time_bucket_at_data_time));
            }

            // Soundness:
            // * `cells[0]` is guaranteed to exist since we passed in `&[component_name]`
            // * `cells[0]` is guaranteed to be non-null, otherwise this whole result would be null
            let Some(cell) = cells[0].take() else {
                debug_assert!(cells[0].is_some(), "unreachable: `cells[0]` is missing");
                return None;
            };

            let bucket = Arc::new(CachedLatestAtComponentResults {
                index: (data_time, row_id),
                promise: Some(Promise::new(cell)),
                cached_dense: Default::default(),
                cached_sparse: Default::default(),
            });

            // Slowest path: this is a complete cache miss.
            {
                re_log::trace!(query_time=?query.at(), ?data_time, "cache miss");

                let query_time_bucket_at_query_time =
                    query_time_bucket_at_query_time.insert(Arc::clone(&bucket));

                let data_time_bucket_at_data_time = per_data_time.entry(data_time);
                data_time_bucket_at_data_time
                    .and_modify(|v| *v = Arc::clone(query_time_bucket_at_query_time))
                    .or_insert(Arc::clone(query_time_bucket_at_query_time));
            }

            Some(bucket)
        } else {
            None
        }
    }

    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            per_query_time,
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
