use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::Arc};

use indexmap::IndexMap;
use itertools::Itertools;
use parking_lot::RwLock;

use re_data_store::{DataStore, LatestAtQuery, TimeInt};
use re_log_types::{EntityPath, RowId};
use re_types::components::ClearIsRecursive;
use re_types::Loggable;
use re_types_core::ComponentName;
use re_types_core::SizeBytes;

use crate::{CacheKey, Caches, LatestAtComponentResults, LatestAtResults, Promise};

// ---

/// Compute the ordering of two data indices, making sure to deal with `STATIC` data appropriately.
//
// TODO(cmc): Maybe at some point we'll want to introduce a dedicated `DataIndex` type with
// proper ordering operators etc.
// It's harder than it sounds though -- depending on the context, you don't necessarily want index
// ordering to behave the same way.
fn compare_indices(lhs: (TimeInt, RowId), rhs: (TimeInt, RowId)) -> std::cmp::Ordering {
    match (lhs, rhs) {
        ((TimeInt::STATIC, lhs_row_id), (TimeInt::STATIC, rhs_row_id)) => {
            lhs_row_id.cmp(&rhs_row_id)
        }
        ((_, _), (TimeInt::STATIC, _)) => std::cmp::Ordering::Less,
        ((TimeInt::STATIC, _), (_, _)) => std::cmp::Ordering::Greater,
        _ => lhs.cmp(&rhs),
    }
}

impl Caches {
    /// Queries for the given `component_names` using latest-at semantics.
    ///
    /// See [`LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at(
        &self,
        store: &DataStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> LatestAtResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = LatestAtResults::default();

        // Query-time clears
        // -----------------
        //
        // We need to find, at query time, whether there exist a `Clear` component that should
        // shadow part or all of the results that we are about to return.
        //
        // This is a two-step process.
        //
        // First, we need to find all `Clear` components that could potentially affect the returned
        // results, i.e. any `Clear` component on the entity itself, or any recursive `Clear`
        // component on any of its recursive parents.
        //
        // Then, we need to compare the index of each component result with the index of the most
        // recent relevant `Clear` component that was found: if there exists a `Clear` component with
        // both a _data time_ lesser or equal to the _query time_ and an index greater or equal
        // than the indexed of the returned data, then we know for sure that the `Clear` shadows
        // the data.
        let mut max_clear_index = (TimeInt::MIN, RowId::ZERO);
        {
            re_tracing::profile_scope!("clears");

            let mut clear_entity_path = entity_path.clone();
            loop {
                let key = CacheKey::new(
                    clear_entity_path.clone(),
                    query.timeline(),
                    ClearIsRecursive::name(),
                );

                let cache = Arc::clone(
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
                );

                let mut cache = cache.write();
                cache.handle_pending_invalidation();
                if let Some(cached) =
                    cache.latest_at(store, query, &clear_entity_path, ClearIsRecursive::name())
                {
                    // When checking the entity itself, any kind of `Clear` component
                    // (i.e. recursive or not) will do.
                    //
                    // For (recursive) parents, we need to deserialize the data to make sure the
                    // recursive flag is set.
                    #[allow(clippy::collapsible_if)] // readability
                    if clear_entity_path == *entity_path
                        || cached.mono::<ClearIsRecursive>(&crate::PromiseResolver {})
                            == Some(ClearIsRecursive(true))
                    {
                        if compare_indices(*cached.index(), max_clear_index)
                            == std::cmp::Ordering::Greater
                        {
                            max_clear_index = *cached.index();
                        }
                    }
                }

                let Some(parent_entity_path) = clear_entity_path.parent() else {
                    break;
                };

                clear_entity_path = parent_entity_path;
            }
        }

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = if crate::cacheable(component_name) {
                Arc::clone(
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
                )
            } else {
                // If the component shouldn't be cached, simply instantiate a new cache for it.
                // It will be dropped when the user is done with it.
                Arc::new(RwLock::new(LatestAtCache::new(key.clone())))
            };

            let mut cache = cache.write();
            cache.handle_pending_invalidation();
            if let Some(cached) = cache.latest_at(store, query, entity_path, component_name) {
                // 1. A `Clear` component doesn't shadow its own self.
                // 2. If a `Clear` component was found with an index greater than or equal to the
                //    component data, then we know for sure that it should shadow it.
                if component_name == ClearIsRecursive::name()
                    || compare_indices(*cached.index(), max_clear_index)
                        == std::cmp::Ordering::Greater
                {
                    results.add(component_name, cached);
                }
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
    pub per_query_time: BTreeMap<TimeInt, Arc<LatestAtComponentResults>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    //
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_data_time: BTreeMap<TimeInt, Arc<LatestAtComponentResults>>,

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
        // NOTE: per query time buckets are just pointers, don't count them.

        let per_data_time_keys = per_data_time
            .keys()
            .map(|k| k.total_size_bytes())
            .sum::<u64>();
        let per_data_time_values = per_data_time
            .values()
            // NOTE: make sure to dereference the Arc, else this will account for zero (assumed amortized!)
            .map(|arc| (**arc).total_size_bytes())
            .sum::<u64>();

        let per_data_time = per_data_time_keys + per_data_time_values;
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
    ) -> Option<Arc<LatestAtComponentResults>> {
        re_tracing::profile_scope!("latest_at", format!("{query:?}"));

        let Self {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let query_time_bucket_at_query_time = match per_query_time.entry(query.at()) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                // Fastest path: we have an entry for this exact query time, no need to look any
                // further.
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

            let bucket = Arc::new(LatestAtComponentResults {
                index: (data_time, row_id),
                promise: Some(Promise::new(cell)),
                cached_dense: Default::default(),
            });

            // Slowest path: this is a complete cache miss.
            {
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

        // First, remove any data indexed by a _query time_ that's more recent than the oldest
        // _data time_ that's been invalidated.
        //
        // Note that this data time might very well be `TimeInt::STATIC`, in which case the entire
        // query-time-based index will be dropped.
        if let Some(&oldest_data_time) = pending_invalidations.first() {
            per_query_time.retain(|&query_time, _| query_time < oldest_data_time);
        }

        // Second, remove any data indexed by _data time_, if it's been invalidated.
        let mut dropped_data_times = Vec::new();
        per_data_time.retain(|data_time, _| {
            if pending_invalidations.contains(data_time) {
                dropped_data_times.push(*data_time);
                false
            } else {
                true
            }
        });

        // TODO(#5974): Because of non-deterministic ordering and parallelism and all things of that
        // nature, it can happen that we try to handle pending invalidations before we even cached
        // the associated data.
        //
        // If that happens, the data will be cached after we've invalidated *nothing*, and will stay
        // there indefinitely since the cache doesn't have a dedicated GC yet.
        //
        // TL;DR: make sure to keep track of pending invalidations indefinitely as long as we
        // haven't had the opportunity to actually invalidate the associated data.
        for data_time in dropped_data_times {
            pending_invalidations.remove(&data_time);
        }
    }
}
