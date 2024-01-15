use std::{collections::BTreeMap, sync::Arc};

use parking_lot::RwLock;
use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, TimeInt};
use re_log_types::{EntityPath, RowId};
use re_query::query_archetype;
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{CacheBucket, Caches, MaybeCachedComponentData};

// --- Data structures ---

/// Caches the results of `LatestAt` queries.
#[derive(Default)]
pub struct LatestAtCache {
    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query and check
    /// if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
    pub per_query_time: BTreeMap<TimeInt, Arc<RwLock<CacheBucket>>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    pub per_data_time: BTreeMap<TimeInt, Arc<RwLock<CacheBucket>>>,

    /// Dedicated bucket for timeless data, if any.
    ///
    /// Query time and data time are one and the same in the timeless case, therefore we only need
    /// this one bucket.
    //
    // NOTE: Lives separately so we don't pay the extra `Option` cost in the much more common
    // timeful case.
    pub timeless: Option<CacheBucket>,

    /// Total size of the data stored in this cache in bytes.
    pub total_size_bytes: u64,
}

// --- Queries ---

macro_rules! impl_query_archetype_latest_at {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_latest_at_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
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
                    MaybeCachedComponentData<'_, InstanceKey>,
                    $(MaybeCachedComponentData<'_, $pov>,)+
                    $(MaybeCachedComponentData<'_, Option<$comp>>,)*
                ),
            ),
        {
            let mut iter_results = |timeless: bool, bucket: &crate::CacheBucket| -> crate::Result<()> {
                re_tracing::profile_scope!("iter");

                let it = itertools::izip!(
                    bucket.iter_data_times(),
                    bucket.iter_pov_instance_keys(),
                    $(bucket.iter_component::<$pov>()
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$pov>::name()))?,)+
                    $(bucket.iter_component_opt::<$comp>()
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$comp>::name()))?,)*
                ).map(|((time, row_id), instance_keys, $($pov,)+ $($comp,)*)| {
                    (
                        ((!timeless).then_some(*time), *row_id),
                        MaybeCachedComponentData::Cached(instance_keys),
                        $(MaybeCachedComponentData::Cached($pov),)+
                        $(MaybeCachedComponentData::Cached($comp),)*
                    )
                });

                for data in it {
                    f(data);
                }


                Ok(())
            };

            let upsert_results = |
                    data_time: TimeInt,
                    arch_view: &::re_query::ArchetypeView<A>,
                    bucket: &mut crate::CacheBucket,
                | -> crate::Result<u64> {
                re_log::trace!(data_time=?data_time, ?data_time, "fill");

                // Grabbing the current time is quite costly on web.
                #[cfg(not(target_arch = "wasm32"))]
                let now = web_time::Instant::now();

                let mut added_size_bytes = 0u64;
                added_size_bytes += bucket.[<insert_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)*>(data_time, &arch_view)?;

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let elapsed = now.elapsed();
                    ::re_log::trace!(
                        store_id=%store.id(),
                        %entity_path,
                        archetype=%A::name(),
                        added_size_bytes,
                        "cached new entry in {elapsed:?} ({:0.3} entries/s)",
                        1f64 / elapsed.as_secs_f64()
                    );
                }

                Ok(added_size_bytes)
            };

            let mut latest_at_callback = |query: &LatestAtQuery, latest_at_cache: &mut crate::LatestAtCache| {
                re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                let crate::LatestAtCache { per_query_time, per_data_time, timeless, total_size_bytes } = latest_at_cache;

                let query_time_bucket_at_query_time = match per_query_time.entry(query.at) {
                    std::collections::btree_map::Entry::Occupied(query_time_bucket_at_query_time) => {
                        // Fastest path: we have an entry for this exact query time, no need to look any
                        // further.
                        re_log::trace!(query_time=?query.at, "cache hit (query time)");
                        return iter_results(false, &query_time_bucket_at_query_time.get().read());
                    }
                    entry => entry,
                };


                let arch_view = query_archetype::<A>(store, &query, entity_path)?;
                let data_time = arch_view.data_time();

                // Fast path: we've run the query and realized that we already have the data for the resulting
                // _data_ time, so let's use that to avoid join & deserialization costs.
                if let Some(data_time) = data_time { // Reminder: `None` means timeless.
                    if let Some(data_time_bucket_at_data_time) = per_data_time.get(&data_time) {
                        re_log::trace!(query_time=?query.at, ?data_time, "cache hit (data time)");

                        *query_time_bucket_at_query_time.or_default() = std::sync::Arc::clone(&data_time_bucket_at_data_time);

                        // We now know for a fact that a query at that data time would yield the same
                        // results: copy the bucket accordingly so that the next cache hit for that query
                        // time ends up taking the fastest path.
                        let query_time_bucket_at_data_time = per_query_time.entry(data_time);
                        *query_time_bucket_at_data_time.or_default() = std::sync::Arc::clone(&data_time_bucket_at_data_time);

                        return iter_results(false, &data_time_bucket_at_data_time.read());
                    }
                } else {
                    if let Some(timeless_bucket) = timeless.as_ref() {
                        re_log::trace!(query_time=?query.at, "cache hit (data time, timeless)");
                        return iter_results(true, timeless_bucket);
                    }
                }


                // Slowest path: this is a complete cache miss.
                if let Some(data_time) = data_time { // Reminder: `None` means timeless.
                    re_log::trace!(query_time=?query.at, ?data_time, "cache miss");

                    // BEWARE: Do _not_ move this out of this scope, or a bucket would be created
                    // even when taking the timeless path!
                    let query_time_bucket_at_query_time = query_time_bucket_at_query_time.or_default();

                    {
                        let mut query_time_bucket_at_query_time = query_time_bucket_at_query_time.write();
                        *total_size_bytes += upsert_results(data_time, &arch_view, &mut query_time_bucket_at_query_time)?;
                    }

                    let data_time_bucket_at_data_time = per_data_time.entry(data_time);
                    *data_time_bucket_at_data_time.or_default() = std::sync::Arc::clone(&query_time_bucket_at_query_time);

                    iter_results(false, &query_time_bucket_at_query_time.read())
                } else {
                    re_log::trace!(query_time=?query.at, "cache miss (timeless)");

                    let mut timeless_bucket = crate::CacheBucket::default();

                    *total_size_bytes += upsert_results(TimeInt::MIN, &arch_view, &mut timeless_bucket)?;
                    iter_results(true, &timeless_bucket)?;

                    *timeless = Some(timeless_bucket);

                    Ok(())
                }
            };


            Caches::with_latest_at::<A, _, _>(
                store.id().clone(),
                entity_path.clone(),
                query,
                |latest_at_cache| latest_at_callback(query, latest_at_cache),
            )
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

seq!(NUM_COMP in 0..10 {
    impl_query_archetype_latest_at!(for N=1, M=NUM_COMP);
});
