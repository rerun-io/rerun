use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, RangeQuery, TimeInt};
use re_log_types::{EntityPath, RowId, TimeRange, Timeline};
use re_types_core::{components::InstanceKey, Archetype, Component, SizeBytes};

use crate::{CacheBucket, Caches, MaybeCachedComponentData};

// --- Data structures ---

/// Caches the results of `Range` queries.
#[derive(Default)]
pub struct RangeCache {
    /// All timeful data, organized by _data_ time.
    ///
    /// Query time is irrelevant for range queries.
    //
    // TODO(#4810): bucketize
    pub per_data_time: CacheBucket,

    /// All timeless data.
    pub timeless: CacheBucket,

    /// For debugging purposes.
    pub(crate) timeline: Timeline,
}

impl std::fmt::Debug for RangeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            per_data_time,
            timeless,
            timeline,
        } = self;

        let mut strings = Vec::new();

        let mut data_time_min = TimeInt::MAX;
        let mut data_time_max = TimeInt::MIN;

        if !timeless.is_empty() {
            data_time_min = TimeInt::MIN;
        }

        if !per_data_time.is_empty() {
            data_time_min = TimeInt::min(
                data_time_min,
                per_data_time.data_times.front().map(|(t, _)| *t).unwrap(),
            );
            data_time_max = TimeInt::max(
                data_time_max,
                per_data_time.data_times.back().map(|(t, _)| *t).unwrap(),
            );
        }

        strings.push(format!(
            "{} ({})",
            timeline
                .typ()
                .format_range_utc(TimeRange::new(data_time_min, data_time_max)),
            re_format::format_bytes(
                (timeless.total_size_bytes + per_data_time.total_size_bytes) as _
            ),
        ));
        strings.push(indent::indent_all_by(2, format!("{timeless:?}")));
        strings.push(indent::indent_all_by(2, format!("{per_data_time:?}")));

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for RangeCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            per_data_time,
            timeless,
            timeline: _,
        } = self;

        per_data_time.total_size_bytes + timeless.total_size_bytes
    }
}

impl RangeCache {
    /// Removes everything from the cache that corresponds to a time equal or greater than the
    /// specified `threshold`.
    ///
    /// Reminder: invalidating timeless data is the same as invalidating everything, so just reset
    /// the `RangeCache` entirely in that case.
    ///
    /// Returns the number of bytes removed.
    #[inline]
    pub fn truncate_at_time(&mut self, threshold: TimeInt) -> u64 {
        let Self {
            per_data_time,
            timeless: _,
            timeline: _,
        } = self;

        per_data_time.truncate_at_time(threshold)
    }
}

impl RangeCache {
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

        if self.per_data_time.is_empty() {
            return Some(reduced_query);
        }

        if let Some(bucket_time_range) = self.per_data_time.time_range() {
            reduced_query.range.max = TimeInt::min(
                reduced_query.range.max,
                bucket_time_range.min.as_i64().saturating_sub(1).into(),
            );
        } else {
            return Some(reduced_query);
        }

        if reduced_query.range.max < reduced_query.range.min {
            return None;
        }

        Some(reduced_query)
    }

    /// Given a `query`, returns a reduced query that is sufficient to fill the missing data
    /// on the back side of the cache, or `None` if all the necessary data is already
    /// cached.
    pub fn compute_back_query(&self, query: &RangeQuery) -> Option<RangeQuery> {
        let mut reduced_query = query.clone();

        if let Some(bucket_time_range) = self.per_data_time.time_range() {
            reduced_query.range.min = TimeInt::max(
                reduced_query.range.min,
                bucket_time_range.max.as_i64().saturating_add(1).into(),
            );
        } else {
            return Some(reduced_query);
        }

        if reduced_query.range.max < reduced_query.range.min {
            return None;
        }

        Some(reduced_query)
    }
}

// --- Queries ---

macro_rules! impl_query_archetype_range {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_range_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            &self,
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
                    $(Option<MaybeCachedComponentData<'_, Option<$comp>>>,)*
                ),
            ),
        {
            let range_results = |
                timeless: bool,
                bucket: &crate::CacheBucket,
                time_range: TimeRange,
                f: &mut F,
            | -> crate::Result<()> {
                re_tracing::profile_scope!("iter");

                let entry_range = bucket.entry_range(time_range);

                let it = itertools::izip!(
                    bucket.range_data_times(entry_range.clone()),
                    bucket.range_pov_instance_keys(entry_range.clone()),
                    $(bucket.range_component::<$pov>(entry_range.clone())
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$pov>::name()))?,)+
                    $(bucket.range_component_opt::<$comp>(entry_range.clone())
                        .map_or_else(
                            || itertools::Either::Left(std::iter::repeat(&[] as &[Option<$comp>])),
                            |it| itertools::Either::Right(it)),
                    )*
                ).map(|((time, row_id), instance_keys, $($pov,)+ $($comp,)*)| {
                    (
                        ((!timeless).then_some(*time), *row_id),
                        MaybeCachedComponentData::Cached(instance_keys),
                        $(MaybeCachedComponentData::Cached($pov),)+
                        $((!$comp.is_empty()).then_some(MaybeCachedComponentData::Cached($comp)),)*
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
                re_tracing::profile_scope!("fill");

                // Grabbing the current time is quite costly on web.
                #[cfg(not(target_arch = "wasm32"))]
                let now = web_time::Instant::now();

                #[cfg(not(target_arch = "wasm32"))]
                let mut added_entries = 0u64;

                let mut added_size_bytes = 0u64;

                for arch_view in arch_views {
                    let data_time = arch_view.data_time().unwrap_or(TimeInt::MIN);

                    if bucket.contains_data_row(data_time, arch_view.primary_row_id()) {
                        continue;
                    }

                    added_size_bytes += bucket.[<insert_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)*>(data_time, &arch_view)?;

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        added_entries += 1;
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                if added_entries > 0 {
                    let elapsed = now.elapsed();
                    ::re_log::trace!(
                        archetype=%A::name(),
                        added_size_bytes,
                        "cached {added_entries} entries in {elapsed:?} ({:0.3} entries/s)",
                        added_entries as f64 / elapsed.as_secs_f64()
                    );
                }

                Ok(added_size_bytes)
            }

            let upsert_callback = |query: &RangeQuery, range_cache: &mut crate::RangeCache| -> crate::Result<()> {
                re_tracing::profile_scope!("range", format!("{query:?}"));

                // NOTE: Same logic as what the store does.
                if query.range.min <= TimeInt::MIN {
                    let mut reduced_query = query.clone();
                    // This is the reduced query corresponding to the timeless part of the data.
                    // It is inclusive and so it will yield `MIN..=MIN` = `[MIN]`.
                    reduced_query.range.max = TimeInt::MIN; // inclusive

                    // NOTE: `+ 1` because we always grab the instance keys.
                    let arch_views = ::re_query::range_component_set::<A, { $N + $M + 1 }>(
                        store, &reduced_query, entity_path,
                        &[$(<$pov>::name(),)+],
                        [<InstanceKey as re_types_core::Loggable>::name(), $(<$pov>::name(),)+ $(<$comp>::name(),)*],
                    );
                    upsert_results::<A, $($pov,)+ $($comp,)*>(arch_views, &mut range_cache.timeless)?;
                }

                let mut query = query.clone();
                query.range.min = TimeInt::max((TimeInt::MIN.as_i64() + 1).into(), query.range.min);

                for reduced_query in range_cache.compute_queries(&query) {
                    // NOTE: `+ 1` because we always grab the instance keys.
                    let arch_views = ::re_query::range_component_set::<A, { $N + $M + 1 }>(
                        store, &reduced_query, entity_path,
                        &[$(<$pov>::name(),)+],
                        [<InstanceKey as re_types_core::Loggable>::name(), $(<$pov>::name(),)+ $(<$comp>::name(),)*],
                    );
                    upsert_results::<A, $($pov,)+ $($comp,)*>(arch_views, &mut range_cache.per_data_time)?;
                }

                Ok(())
            };

            let iter_callback = |query: &RangeQuery, range_cache: &crate::RangeCache, f: &mut F| -> crate::Result<()> {
                re_tracing::profile_scope!("range", format!("{query:?}"));

                // We don't bother implementing the slow path here (busy write lock), as that would
                // require adding a bunch more complexity in order to know whether a range query is
                // already cached (how can you know whether `TimeInt::MAX` is cached? you need to
                // clamp queries based on store metadata first, etc).
                //
                // We can add the extra complexity if this proves to be glitchy in real-world
                // scenarios -- otherwise all of this is giant hack meant to go away anyhow.

                // NOTE: Same logic as what the store does.
                if query.range.min <= TimeInt::MIN {
                    let mut reduced_query = query.clone();
                    // This is the reduced query corresponding to the timeless part of the data.
                    // It is inclusive and so it will yield `MIN..=MIN` = `[MIN]`.
                    reduced_query.range.max = TimeInt::MIN; // inclusive

                    if !range_cache.timeless.is_empty() {
                        range_results(true, &range_cache.timeless, reduced_query.range, f)?;
                    }
                }

                let mut query = query.clone();
                query.range.min = TimeInt::max((TimeInt::MIN.as_i64() + 1).into(), query.range.min);

                if !range_cache.per_data_time.is_empty() {
                    range_results(false, &range_cache.per_data_time, query.range, f)?;
                }

                Ok(())
            };

            let (res1, res2) = self.with_range::<A, _, _, _, _>(
                store,
                entity_path.clone(),
                query,
                |range_cache| upsert_callback(query, range_cache),
                |range_cache| iter_callback(query, range_cache, &mut f),
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
            impl_query_archetype_range!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

impl Caches {
    seq!(NUM_COMP in 0..10 {
        impl_query_archetype_range!(for N=1, M=NUM_COMP);
    });
}
