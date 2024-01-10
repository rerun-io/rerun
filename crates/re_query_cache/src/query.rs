use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, TimeInt, Timeline};
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
            (TimeInt, RowId),
            MaybeCachedComponentData<'_, InstanceKey>,
            MaybeCachedComponentData<'_, R1>,
        ),
    ),
{
    query_archetype_pov1_comp0::<A, R1, F>(store, query, entity_path, f)
}

macro_rules! impl_query_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
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
                    (TimeInt, RowId),
                    MaybeCachedComponentData<'_, InstanceKey>,
                    $(MaybeCachedComponentData<'_, $pov>,)+
                    $(MaybeCachedComponentData<'_, Option<$comp>>,)*
                ),
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype",
                format!("arch={} pov={} comp={}", A::name(), $N, $M)
            );

            let mut latest_at_callback = |query: &LatestAtQuery, cache: &mut crate::LatestAtCache| {
                re_tracing::profile_scope!("latest_at");

                let bucket = cache.entry(query.at).or_default();
                // NOTE: Implicitly dropping the write guard here: the LatestAtCache is free once again!

                if bucket.is_empty() {
                    re_tracing::profile_scope!("fill");

                    let now = web_time::Instant::now();
                    let arch_view = query_archetype::<A>(store, &query, entity_path)?;

                    bucket.[<insert_pov $N _comp$M>]::<A, $($pov,)+ $($comp,)*>(query.at, &arch_view)?;

                    let elapsed = now.elapsed();
                    ::re_log::trace!(
                        store_id=%store.id(),
                        %entity_path,
                        archetype=%A::name(),
                        "cached new entry in {elapsed:?} ({:0.3} entries/s)",
                        1f64 / elapsed.as_secs_f64()
                    );
                }

                re_tracing::profile_scope!("iter");

                let it = itertools::izip!(
                    bucket.iter_pov_times(),
                    bucket.iter_pov_instance_keys(),
                    $(bucket.iter_component::<$pov>()
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$pov>::name()))?,)+
                    $(bucket.iter_component_opt::<$comp>()
                        .ok_or_else(|| re_query::ComponentNotFoundError(<$comp>::name()))?,)*
                ).map(|(time, instance_keys, $($pov,)+ $($comp,)*)| {
                    (
                        *time,
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

            match &query {
                AnyQuery::LatestAt(query) => {
                    Caches::with_latest_at::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |cache| latest_at_callback(query, cache),
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
            (TimeInt, RowId),
            MaybeCachedComponentData<'_, InstanceKey>,
            MaybeCachedComponentData<'_, R1>,
        ),
    ),
{
    query_archetype_with_history_pov1_comp0::<A, R1, F>(store, timeline, time, history, ent_path, f)
}

/// Generates a function to cache a (potentially historical) query with N point-of-view components and M
/// other components.
macro_rules! impl_query_archetype_with_history {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype_with_history`] for `" $N "` point-of-view"]
        #[doc = "components and `" $M "` optional components."]
        pub fn [<query_archetype_with_history_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
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
                    (TimeInt, RowId),
                    MaybeCachedComponentData<'_, InstanceKey>,
                    $(MaybeCachedComponentData<'_, $pov>,)+
                    $(MaybeCachedComponentData<'_, Option<$comp>>,)*
                ),
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype_with_history",
                format!("arch={} pov={} comp={}", A::name(), $N, $M)
            );

            let visible_history = match timeline.typ() {
                re_log_types::TimeType::Time => history.nanos,
                re_log_types::TimeType::Sequence => history.sequences,
            };

            if !history.enabled || visible_history == VisibleHistory::OFF {
                let query = LatestAtQuery::new(*timeline, *time);
                $crate::[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            } else {
                unimplemented!("TODO(cmc): range support");
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
