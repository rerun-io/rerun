use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_entity_db::{ExtraQueryHistory, VisibleHistory};
use re_log_types::{EntityPath, RowId};
use re_query::query_archetype;
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{AnyQuery, Caches};

// ---

/// Either a reference to a slice of data residing in the cache, or some data being deserialized
/// just-in-time from an [`re_query::ArchetypeView`].
#[derive(Debug, Clone)]
pub enum MaybeCachedComponentData<'a, C> {
    Cached(&'a [C]),
    // TODO(cmc): Ideally, this would be a reference to a `dyn Iterator` that is the result of
    // calling `ArchetypeView::iter_{required|optional}_component`.
    // In practice this enters lifetime invariance hell for, from what I can see, no particular gains
    // (rustc is pretty good at optimizing out collections into obvious temporary variables).
    Raw(Vec<C>),
}

impl<'a, C> std::ops::Deref for MaybeCachedComponentData<'a, C> {
    type Target = [C];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, C> MaybeCachedComponentData<'a, C> {
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &C> + '_ {
        self.as_slice().iter()
    }

    #[inline]
    pub fn as_slice(&self) -> &[C] {
        match self {
            MaybeCachedComponentData::Cached(data) => data,
            MaybeCachedComponentData::Raw(data) => data.as_slice(),
        }
    }
}

// ---

/// Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]
/// (combined) for 1 point-of-view component and no optional components.
///
/// Alias for [`query_archetype_pov1_comp0`].
#[inline]
pub fn query_archetype_pov1<'a, A, R1, F>(
    cached: bool,
    store: &'a DataStore,
    query: &AnyQuery,
    entity_path: &'a EntityPath,
    f: F,
) -> ::re_query::Result<()>
where
    A: Archetype + 'a,
    R1: Component + Send + Sync + 'static,
    F: FnMut(
        (
            (Option<TimeInt>, RowId),
            MaybeCachedComponentData<'_, InstanceKey>,
            MaybeCachedComponentData<'_, R1>,
        ),
    ),
{
    query_archetype_pov1_comp0::<A, R1, F>(cached, store, query, entity_path, f)
}

macro_rules! impl_query_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            cached: bool,
            store: &'a DataStore,
            query: &AnyQuery,
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
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype",
                format!("cached={cached} arch={} pov={} comp={}", A::name(), $N, $M)
            );


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
                | -> crate::Result<()> {
                re_log::trace!(data_time=?data_time, ?data_time, "fill");

                // Grabbing the current time is quite costly on web.
                #[cfg(not(target_arch = "wasm32"))]
                let now = web_time::Instant::now();

                bucket.[<insert_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)*>(data_time, &arch_view)?;

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let elapsed = now.elapsed();
                    ::re_log::trace!(
                        store_id=%store.id(),
                        %entity_path,
                        archetype=%A::name(),
                        "cached new entry in {elapsed:?} ({:0.3} entries/s)",
                        1f64 / elapsed.as_secs_f64()
                    );
                }

                Ok(())
            };

            let mut latest_at_callback = |query: &LatestAtQuery, latest_at_cache: &mut crate::LatestAtCache| {
                re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                let crate::LatestAtCache { per_query_time, per_data_time, timeless } = latest_at_cache;

                let query_time_bucket_at_query_time = match per_query_time.entry(query.at) {
                    std::collections::btree_map::Entry::Occupied(query_time_bucket_at_query_time) => {
                        // Fastest path: we have an entry for this exact query time, no need to look any
                        // further.
                        re_log::trace!(query_time=?query.at, "cache hit (query time)");
                        return iter_results(false, &query_time_bucket_at_query_time.get().read());
                    }
                    entry @ std::collections::btree_map::Entry::Vacant(_) => entry,
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

                let query_time_bucket_at_query_time = query_time_bucket_at_query_time.or_default();

                // Slowest path: this is a complete cache miss.
                if let Some(data_time) = data_time { // Reminder: `None` means timeless.
                    re_log::trace!(query_time=?query.at, ?data_time, "cache miss");

                    {
                        let mut query_time_bucket_at_query_time = query_time_bucket_at_query_time.write();
                        upsert_results(data_time, &arch_view, &mut query_time_bucket_at_query_time)?;
                    }

                    let data_time_bucket_at_data_time = per_data_time.entry(data_time);
                    *data_time_bucket_at_data_time.or_default() = std::sync::Arc::clone(&query_time_bucket_at_query_time);

                    iter_results(false, &query_time_bucket_at_query_time.read())
                } else {
                    re_log::trace!(query_time=?query.at, "cache miss (timeless)");

                    let mut timeless_bucket = crate::CacheBucket::default();

                    upsert_results(TimeInt::MIN, &arch_view, &mut timeless_bucket)?;
                    iter_results(true, &timeless_bucket)?;

                    *timeless = Some(timeless_bucket);
                    Ok(())
                }
            };


            match &query {
                // TODO(cmc): cached range support
                AnyQuery::Range(query) => {
                    re_tracing::profile_scope!("range", format!("{query:?}"));

                    // NOTE: `+ 2` because we always grab the indicator component as well as the
                    // instance keys.
                    let arch_views = ::re_query::range_archetype::<A, { $N + $M + 2 }>(store, query, entity_path);

                    for arch_view in arch_views {
                        let data = (
                            (arch_view.data_time(), arch_view.primary_row_id()),
                            MaybeCachedComponentData::Raw(arch_view.iter_instance_keys().collect()),
                            $(MaybeCachedComponentData::Raw(arch_view.iter_required_component::<$pov>()?.collect()),)+
                            $(MaybeCachedComponentData::Raw(arch_view.iter_optional_component::<$comp>()?.collect()),)*
                        );

                        f(data);
                    }

                    Ok(())
                }

                AnyQuery::LatestAt(query) if !cached => {
                    re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                    let arch_view = ::re_query::query_archetype::<A>(store, query, entity_path)?;

                    let data = (
                        (arch_view.data_time(), arch_view.primary_row_id()),
                        MaybeCachedComponentData::Raw(arch_view.iter_instance_keys().collect()),
                        $(MaybeCachedComponentData::Raw(arch_view.iter_required_component::<$pov>()?.collect()),)+
                        $(MaybeCachedComponentData::Raw(arch_view.iter_optional_component::<$comp>()?.collect()),)*
                    );

                    f(data);

                    Ok(())
                }

                AnyQuery::LatestAt(query) => {
                    Caches::with_latest_at::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |latest_at_cache| latest_at_callback(query, latest_at_cache),
                    )
                },
            }
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_archetype!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_archetype!(for N=1, M=NUM_COMP);
});

// ---

/// Cached implementation of [`re_query::query_archetype_with_history`] for 1 point-of-view component
/// and no optional components.
///
/// Alias for [`query_archetype_with_history_pov1_comp0`].
#[inline]
pub fn query_archetype_with_history_pov1<'a, A, R1, F>(
    cached: bool,
    store: &'a DataStore,
    timeline: &'a Timeline,
    time: &'a TimeInt,
    history: &ExtraQueryHistory,
    ent_path: &'a EntityPath,
    f: F,
) -> ::re_query::Result<()>
where
    A: Archetype + 'a,
    R1: Component + Send + Sync + 'static,
    F: FnMut(
        (
            (Option<TimeInt>, RowId),
            MaybeCachedComponentData<'_, InstanceKey>,
            MaybeCachedComponentData<'_, R1>,
        ),
    ),
{
    query_archetype_with_history_pov1_comp0::<A, R1, F>(
        cached, store, timeline, time, history, ent_path, f,
    )
}

/// Generates a function to cache a (potentially historical) query with N point-of-view components and M
/// other components.
macro_rules! impl_query_archetype_with_history {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype_with_history`] for `" $N "` point-of-view"]
        #[doc = "components and `" $M "` optional components."]
        pub fn [<query_archetype_with_history_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            cached: bool,
            store: &'a DataStore,
            timeline: &'a Timeline,
            time: &'a TimeInt,
            history: &ExtraQueryHistory,
            ent_path: &'a EntityPath,
            f: F,
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
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype_with_history",
                format!("cached={cached} arch={} pov={} comp={}", A::name(), $N, $M)
            );

            let visible_history = match timeline.typ() {
                re_log_types::TimeType::Time => history.nanos,
                re_log_types::TimeType::Sequence => history.sequences,
            };

            if !history.enabled || visible_history == VisibleHistory::OFF {
                let query = LatestAtQuery::new(*timeline, *time);
                $crate::[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    cached,
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            } else {
                let min_time = visible_history.from(*time);
                let max_time = visible_history.to(*time);
                let query = RangeQuery::new(*timeline, TimeRange::new(min_time, max_time));
                $crate::[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    cached,
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            }
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_archetype_with_history!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_archetype_with_history!(for N=1, M=NUM_COMP);
});
