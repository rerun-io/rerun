use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, TimeInt};
use re_log_types::{EntityPath, RowId, Timeline};
use re_query::query_archetype;
use re_types_core::{components::InstanceKey, Archetype, Component, SizeBytes};

use crate::{CacheBucket, Caches};

// --- Data structures ---

/// Caches the results of `LatestAt` queries.
#[derive(Default)]
pub struct LatestAtCache {
    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query and check
    /// if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
    //
    // NOTE: `Arc` so we can deduplicate buckets across query time & data time.
    pub per_query_time: BTreeMap<TimeInt, Arc<CacheBucket>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    //
    // NOTE: `Arc` so we can deduplicate buckets across query time & data time.
    pub per_data_time: BTreeMap<TimeInt, Arc<CacheBucket>>,

    /// Dedicated bucket for timeless data, if any.
    ///
    /// Query time and data time are one and the same in the timeless case, therefore we only need
    /// this one bucket.
    //
    // NOTE: Lives separately so we don't pay the extra `Option` cost in the much more common
    // timeful case.
    pub timeless: Option<Arc<CacheBucket>>,

    /// For debugging purposes.
    pub(crate) timeline: Timeline,

    /// Total size of the data stored in this cache in bytes.
    total_size_bytes: u64,
}

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
            strings.push(indent::indent_all_by(2, format!("{bucket:?}")));
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

// --- Queries ---

macro_rules! impl_query_archetype_latest_at {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_latest_at_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            &self,
            store: &'a DataStore,
            query: &LatestAtQuery,
            entity_path: &'a EntityPath,
            mut f: F,
        ) -> ::re_query::Result<()>
        where
            A: Archetype + 'a,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
            F: FnMut(
                (
                    (Option<TimeInt>, RowId),
                    &[InstanceKey],
                    $(&[$pov],)+
                    $(Option<&[Option<$comp>]>,)*
                ),
            ),
        {
            let iter_results = |timeless: bool, bucket: &crate::CacheBucket, f: &mut F| -> crate::Result<()> {
                // Profiling this in isolation can be useful, but adds a lot of noise for small queries.
                //re_tracing::profile_scope!("iter");

                let it = itertools::izip!(
                    bucket.iter_data_times(),
                    bucket.iter_pov_instance_keys(),
                    $(bucket.iter_component::<$pov>()
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$pov>::name()))?,)+
                    $(bucket.iter_component_opt::<$comp>()
                        .map_or_else(
                            || itertools::Either::Left(std::iter::repeat(&[] as &[Option<$comp>])),
                            |it| itertools::Either::Right(it)),
                    )*
                ).map(|((time, row_id), instance_keys, $($pov,)+ $($comp,)*)| {
                    (
                        ((!timeless).then_some(*time), *row_id),
                        instance_keys,
                        $($pov,)+
                        $((!$comp.is_empty()).then_some($comp),)*
                    )
                });

                for data in it {
                    f(data);
                }

                Ok(())
            };

            let create_and_fill_bucket = |
                data_time: TimeInt,
                arch_view: &::re_query::ArchetypeView<A>,
            | -> crate::Result<crate::CacheBucket> {
                re_log::trace!(data_time=?data_time, ?data_time, "fill");

                // Grabbing the current time is quite costly on web.
                #[cfg(not(target_arch = "wasm32"))]
                let now = web_time::Instant::now();

                let mut bucket = crate::CacheBucket::default();
                bucket.[<insert_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)*>(data_time, &arch_view)?;

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let elapsed = now.elapsed();
                    ::re_log::trace!(
                        store_id=%store.id(),
                        %entity_path,
                        archetype=%A::name(),
                        added_size_bytes=bucket.total_size_bytes,
                        "cached new entry in {elapsed:?} ({:0.3} entries/s)",
                        1f64 / elapsed.as_secs_f64()
                    );
                }

                Ok(bucket)
            };

            let upsert_callback = |query: &LatestAtQuery, latest_at_cache: &mut crate::LatestAtCache| -> crate::Result<()> {
                re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                let crate::LatestAtCache {
                    per_query_time,
                    per_data_time,
                    timeless,
                    timeline: _,
                    total_size_bytes,
                } = latest_at_cache;

                let query_time_bucket_at_query_time = match per_query_time.entry(query.at()) {
                    std::collections::btree_map::Entry::Occupied(_) => {
                        // Fastest path: we have an entry for this exact query time, no need to look any
                        // further.
                        re_log::trace!(query_time=?query.at(), "cache hit (query time)");
                        return Ok(());
                    }
                    std::collections::btree_map::Entry::Vacant(entry) => entry,
                };

                let arch_view = query_archetype::<A>(store, &query, entity_path)?;
                let data_time = arch_view.data_time();

                // Fast path: we've run the query and realized that we already have the data for the resulting
                // _data_ time, so let's use that to avoid join & deserialization costs.
                if let Some(data_time) = data_time { // Reminder: `None` means timeless.
                    if let Some(data_time_bucket_at_data_time) = per_data_time.get(&data_time) {
                        re_log::trace!(query_time=?query.at(), ?data_time, "cache hit (data time)");

                        query_time_bucket_at_query_time.insert(Arc::clone(&data_time_bucket_at_data_time));

                        // We now know for a fact that a query at that data time would yield the same
                        // results: copy the bucket accordingly so that the next cache hit for that query
                        // time ends up taking the fastest path.
                        let query_time_bucket_at_data_time = per_query_time.entry(data_time);
                        query_time_bucket_at_data_time
                            .and_modify(|v| *v = Arc::clone(&data_time_bucket_at_data_time))
                            .or_insert(Arc::clone(&data_time_bucket_at_data_time));

                        return Ok(());
                    }
                } else {
                    if let Some(timeless) = timeless.as_ref() {
                        re_log::trace!(query_time=?query.at(), "cache hit (data time, timeless)");
                        query_time_bucket_at_query_time.insert(Arc::clone(timeless));
                        return Ok(());
                    }
                }

                // Slowest path: this is a complete cache miss.
                if let Some(data_time) = data_time { // Reminder: `None` means timeless.
                    re_log::trace!(query_time=?query.at(), ?data_time, "cache miss");

                    let bucket = Arc::new(create_and_fill_bucket(data_time, &arch_view)?);
                    *total_size_bytes += bucket.total_size_bytes;
                    let query_time_bucket_at_query_time = query_time_bucket_at_query_time.insert(bucket);

                    let data_time_bucket_at_data_time = per_data_time.entry(data_time);
                    data_time_bucket_at_data_time
                        .and_modify(|v| *v = Arc::clone(&query_time_bucket_at_query_time))
                        .or_insert(Arc::clone(&query_time_bucket_at_query_time));

                    Ok(())
                } else {
                    re_log::trace!(query_time=?query.at(), "cache miss (timeless)");

                    let bucket = create_and_fill_bucket(TimeInt::MIN, &arch_view)?;
                    *total_size_bytes += bucket.total_size_bytes;

                    let bucket = Arc::new(bucket);
                    *timeless = Some(Arc::clone(&bucket));
                    query_time_bucket_at_query_time.insert(Arc::clone(&bucket));

                    Ok(())
                }
            };

            let iter_callback = |query: &LatestAtQuery, latest_at_cache: &crate::LatestAtCache, f: &mut F| {
                re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                let crate::LatestAtCache {
                    per_query_time,
                    per_data_time: _,
                    timeless,
                    timeline: _,
                    total_size_bytes: _,
                } = latest_at_cache;

                // Expected path: cache was properly upserted.
                if let Some(query_time_bucket_at_query_time) = per_query_time.get(&query.at()) {
                    let is_timeless = std::ptr::eq(
                        Arc::as_ptr(query_time_bucket_at_query_time),
                        timeless.as_ref().map_or(std::ptr::null(), |bucket| Arc::as_ptr(bucket)),
                    );
                    return iter_results(is_timeless, query_time_bucket_at_query_time, f);
                }

                re_log::trace!(
                    store_id = %store.id(),
                    %entity_path,
                    ?query,
                    "either no data exist at this time or we couldn't upsert the cache (write lock was busy)"
                );

                Ok(())
            };


            let (res1, res2) = self.with_latest_at::<A, _, _, _, _>(
                store,
                entity_path.clone(),
                query,
                |latest_at_cache| upsert_callback(query, latest_at_cache),
                |latest_at_cache| iter_callback(query, latest_at_cache, &mut f),
            );

            if let Some(res1) = res1 {
                res1?;
            }
            res2?;

            Ok(())
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_archetype_latest_at!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

impl Caches {
    seq!(NUM_COMP in 0..10 {
        impl_query_archetype_latest_at!(for N=1, M=NUM_COMP);
    });
}
