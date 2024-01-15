use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, RangeQuery, TimeInt};
use re_log_types::{EntityPath, RowId};
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{CacheBucket, Caches, MaybeCachedComponentData};

// --- Data structures ---

/// Caches the results of `Range` queries.
#[derive(Default)]
pub struct RangeCache {
    // TODO(cmc): bucketize
    pub bucket: CacheBucket,

    /// Total size of the data stored in this cache in bytes.
    pub total_size_bytes: u64,
}

// --- Queries ---

macro_rules! impl_query_archetype_range {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_range_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            store: &'a DataStore,
            query: &RangeQuery,
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
            let mut iter_results = |bucket: &crate::CacheBucket| -> crate::Result<()> {
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
                        (Some(*time), *row_id), // TODO(cmc): timeless
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

            fn upsert_results<'a, A, $($pov,)+ $($comp,)*>(
                arch_views: impl Iterator<Item = re_query::ArchetypeView<A>>,
                bucket: &mut crate::CacheBucket,
            ) -> crate::Result<u64>
            where
                A: Archetype + 'a,
                $($pov: Component + Send + Sync + 'static,)+
                $($comp: Component + Send + Sync + 'static,)*
            {
                re_log::trace!("fill");

                let now = web_time::Instant::now();

                let mut added_entries = 0u64;
                let mut added_size_bytes = 0u64;

                for arch_view in arch_views {
                    let data_time = arch_view.data_time().unwrap_or(TimeInt::MIN); // TODO(cmc): timeless

                    if bucket.contains_data_row(data_time, arch_view.primary_row_id()) {
                        continue;
                    }

                    added_size_bytes += bucket.[<insert_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)*>(data_time, &arch_view)?;
                    added_entries += 1;
                }

                let elapsed = now.elapsed();
                ::re_log::trace!(
                    archetype=%A::name(),
                    added_size_bytes,
                    "cached {added_entries} entries in {elapsed:?} ({:0.3} entries/s)",
                    added_entries as f64 / elapsed.as_secs_f64()
                );

                Ok(added_size_bytes)
            }

            let mut range_callback = |query: &RangeQuery, range_cache: &mut crate::RangeCache| {
                re_tracing::profile_scope!("range", format!("{query:?}"));

                let RangeCache { bucket, total_size_bytes } = range_cache;

                // NOTE: `+ 2` because we always grab the indicator component as well as the
                // instance keys.
                let arch_views = ::re_query::range_archetype::<A, { $N + $M + 2 }>(store, query, entity_path);
                *total_size_bytes += upsert_results::<A, $($pov,)+ $($comp,)*>(arch_views, bucket)?;

                iter_results(bucket)
            };


            Caches::with_range::<A, _, _>(
                store.id().clone(),
                entity_path.clone(),
                query,
                |range_cache| range_callback(query, range_cache),
            )
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_archetype_range!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_archetype_range!(for N=1, M=NUM_COMP);
});
