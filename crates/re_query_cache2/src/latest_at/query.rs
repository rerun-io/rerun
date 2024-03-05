use std::{collections::BTreeMap, sync::Arc};

use nohash_hasher::IntSet;

use re_data_store::{DataStore, LatestAtQuery, TimeInt};
use re_log_types::{EntityPath, Timeline};
use re_query2::Promise;
use re_types_core::ComponentName;
use re_types_core::SizeBytes;

use crate::{CacheKey, CachedLatestAtComponentResults, CachedLatestAtResults, Caches};

// --- Data structures ---

impl Caches {
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
            let key = CacheKey::new(entity_path.clone(), query.timeline, component_name);

            let cache = {
                let cache = Arc::clone(self.per_cache_key.write().entry(key.clone()).or_default());
                // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

                //TODO
                // let removed_bytes = caches_per_archetype.write().handle_pending_invalidation();
                // Implicitly releasing archetype-level cache mappings -- concurrent queries using the
                // same `CacheKey` but a different `ArchetypeName` can run once again.
                // if removed_bytes > 0 {
                //     re_log::trace!(
                //         store_id=%self.store_id,
                //         entity_path = %key.entity_path,
                //         removed = removed_bytes,
                //         "invalidated latest-at caches"
                //     );
                // }

                // let caches_per_archetype = caches_per_archetype.read();
                // let mut latest_at_per_archetype =
                //     caches_per_archetype.latest_at_per_archetype.write();
                // Arc::clone(latest_at_per_archetype.entry(A::name()).or_default())
                // // Implicitly releasing bottom-level cache mappings -- identical concurrent queries
                // // can run once again.

                cache
            };

            let mut cache = cache.write();
            cache.handle_pending_invalidation(); // TODO
            if let Some(cached) = cache.latest_at(store, query, entity_path, component_name) {
                results.add(component_name, cached);
            }
        }

        results
    }
}

// TODO: therefore this is entity/timeline/component?

/// Caches the results of `LatestAt` queries.
#[derive(Default)]
pub struct LatestAtCache {
    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query and check
    /// if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
    //
    // NOTE: `Arc` so we can deduplicate buckets across query time & data time.
    pub per_query_time: BTreeMap<TimeInt, Arc<CachedLatestAtComponentResults>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    //
    // NOTE: `Arc` so we can deduplicate buckets across query time & data time.
    pub per_data_time: BTreeMap<TimeInt, Arc<CachedLatestAtComponentResults>>,

    /// Dedicated bucket for timeless data, if any.
    ///
    /// Query time and data time are one and the same in the timeless case, therefore we only need
    /// this one bucket.
    //
    // NOTE: Lives separately so we don't pay the extra `Option` cost in the much more common
    // timeful case.
    pub timeless: Option<Arc<CachedLatestAtComponentResults>>,

    /// For debugging purposes.
    pub timeline: Timeline,

    /// Everything greater than or equal to this timestamp has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching this criteria.
    /// `None` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_timeful_invalidation: IntSet<TimeInt>,

    /// If `true`, the timeless data associated with this cache has been asynchronously invalidated.
    ///
    /// If `true`, this cache must remove all of its timeless entries the next time it gets queried.
    /// `false` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_timeless_invalidation: bool,

    /// Total size of the data stored in this cache in bytes.
    pub total_size_bytes: u64,
    //
    // TODO: just store the cache key in here really
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

        let crate::LatestAtCache {
            per_query_time,
            per_data_time,
            timeless,
            timeline: _,
            pending_timeless_invalidation: _,
            pending_timeful_invalidation: _,
            total_size_bytes,
        } = self;

        let query_time_bucket_at_query_time = match per_query_time.entry(query.at) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                // Fastest path: we have an entry for this exact query time, no need to look any
                // further.
                re_log::trace!(query_time=?query.at, "cache hit (query time)");
                return Some(Arc::clone(entry.get()));
            }
            std::collections::btree_map::Entry::Vacant(entry) => entry,
        };

        let result = store.latest_at(query, entity_path, component_name, &[component_name]);

        // TODO: should we store the absence? do we need to?

        // TODO: cannot use a .map, borrowck gets lost because we keep the entry across.
        if let Some((data_time, row_id, mut cells)) = result {
            // Fast path: we've run the query and realized that we already have the data for the resulting
            // _data_ time, so let's use that to avoid join & deserialization costs.
            if let Some(data_time) = data_time {
                // Reminder: `None` means timeless.
                if let Some(data_time_bucket_at_data_time) = per_data_time.get(&data_time) {
                    re_log::trace!(query_time=?query.at, ?data_time, "cache hit (data time)");

                    query_time_bucket_at_query_time
                        .insert(Arc::clone(data_time_bucket_at_data_time));

                    // We now know for a fact that a query at that data time would yield the same
                    // results: copy the bucket accordingly so that the next cache hit for that query
                    // time ends up taking the fastest path.
                    let query_time_bucket_at_data_time = per_query_time.entry(data_time);
                    query_time_bucket_at_data_time
                        .and_modify(|v| *v = Arc::clone(data_time_bucket_at_data_time))
                        .or_insert(Arc::clone(data_time_bucket_at_data_time));

                    return Some(Arc::clone(data_time_bucket_at_data_time));
                }
            } else {
                if let Some(timeless) = timeless.as_ref() {
                    re_log::trace!(query_time=?query.at, "cache hit (data time, timeless)");
                    query_time_bucket_at_query_time.insert(Arc::clone(timeless));
                    return Some(Arc::clone(timeless));
                }
            }

            let cell = {
                // - `cells[0]` is guaranteed to exist since we passed `&[component_name]`
                // - `cells[0]` is guaranteed to be non-null, otherwise the whole result would be null
                cells[0].take().unwrap()
            };

            let bucket = Arc::new(CachedLatestAtComponentResults {
                index: (data_time, row_id),
                cell: Some(Promise::new(cell)),
                cached: Default::default(),
            });
            // TODO: i guess we update this as part of the once lock
            // *total_size_bytes += bucket.total_size_bytes;

            // Slowest path: this is a complete cache miss.
            if let Some(data_time) = data_time {
                // Reminder: `None` means timeless.
                re_log::trace!(query_time=?query.at, ?data_time, "cache miss");

                let query_time_bucket_at_query_time =
                    query_time_bucket_at_query_time.insert(Arc::clone(&bucket));

                let data_time_bucket_at_data_time = per_data_time.entry(data_time);
                data_time_bucket_at_data_time
                    .and_modify(|v| *v = Arc::clone(query_time_bucket_at_query_time))
                    .or_insert(Arc::clone(query_time_bucket_at_query_time));
            } else {
                re_log::trace!(query_time=?query.at, "cache miss (timeless)");

                *timeless = Some(Arc::clone(&bucket));
                query_time_bucket_at_query_time.insert(Arc::clone(&bucket));
            }

            Some(bucket)
        } else {
            None
        }
    }

    // TODO
    pub fn handle_pending_invalidation(&mut self) -> u64 {
        let Self {
            per_query_time,
            per_data_time,
            timeless,
            timeline: _,
            pending_timeless_invalidation,
            pending_timeful_invalidation,
            total_size_bytes,
        } = self;

        let mut removed_bytes = 0u64; // TODO

        if *pending_timeless_invalidation {
            // TODO: size
            *timeless = None;
        }

        if !pending_timeful_invalidation.is_empty() {
            // TODO: size

            let min_time = pending_timeful_invalidation
                .iter()
                .min()
                .unwrap_or(&TimeInt::MAX);
            per_query_time.retain(|&query_time, _| query_time < *min_time);

            per_data_time.retain(|data_time, _| !pending_timeful_invalidation.contains(data_time));
        }

        *total_size_bytes = total_size_bytes
            .checked_sub(removed_bytes)
            .unwrap_or_else(|| {
                re_log::debug!(
                    current = *total_size_bytes,
                    removed = removed_bytes,
                    "book keeping underflowed"
                );
                u64::MIN
            });

        removed_bytes
    }
}

#[cfg(target_os = "TODO")]
impl std::fmt::Debug for LatestAtCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            per_query_time,
            per_data_time,
            timeless,
            timeline,
            total_size_bytes: _,
        } = self;

        let mut strings = Vec::new();

        if let Some(bucket) = timeless.as_ref() {
            strings.push(format!(
                "query_time=<timeless> -> data_time=<timeless> ({})",
                re_format::format_bytes(bucket.total_size_bytes as _),
            ));
        }

        let data_times_per_bucket: HashMap<_, _> = per_data_time
            .iter()
            .map(|(time, bucket)| (Arc::as_ptr(bucket), *time))
            .collect();

        for (query_time, bucket) in per_query_time {
            let query_time = timeline.typ().format_utc(*query_time);
            let data_time = data_times_per_bucket
                .get(&Arc::as_ptr(bucket))
                .map_or_else(|| "MISSING?!".to_owned(), |t| timeline.typ().format_utc(*t));
            strings.push(format!(
                "query_time={query_time} -> data_time={data_time} ({})",
                re_format::format_bytes(bucket.total_size_bytes as _),
            ));
            // TODO
            // strings.push(indent::indent_all_by(2, format!("{bucket:?}")));
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for LatestAtCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }
}

#[cfg(target_os = "TODO")]
impl LatestAtCache {
    /// Removes everything from the cache that corresponds to a time equal or greater than the
    /// specified `threshold`.
    ///
    /// Reminder: invalidating timeless data is the same as invalidating everything, so just reset
    /// the `LatestAtCache` entirely in that case.
    ///
    /// Returns the number of bytes removed.
    #[inline]
    pub fn truncate_at_time(&mut self, threshold: TimeInt) -> u64 {
        let Self {
            per_query_time,
            per_data_time,
            timeless: _,
            timeline: _,
            total_size_bytes,
        } = self;

        let mut removed_bytes = 0u64;

        per_query_time.retain(|&query_time, _| query_time < threshold);

        // Buckets for latest-at queries are guaranteed to only ever contain a single entry, so
        // just remove the buckets entirely directly.
        per_data_time.retain(|&data_time, bucket| {
            if data_time < threshold {
                return true;
            }

            // Only if that bucket is about to be dropped.
            if Arc::strong_count(bucket) == 1 {
                removed_bytes += bucket.total_size_bytes;
            }

            false
        });

        *total_size_bytes = total_size_bytes
            .checked_sub(removed_bytes)
            .unwrap_or_else(|| {
                re_log::debug!(
                    current = *total_size_bytes,
                    removed = removed_bytes,
                    "book keeping underflowed"
                );
                u64::MIN
            });

        removed_bytes
    }
}
