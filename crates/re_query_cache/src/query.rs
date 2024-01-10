use paste::paste;
use seq_macro::seq;

use re_arrow_store::{DataStore, LatestAtQuery, TimeInt, Timeline};
use re_data_store::{ExtraQueryHistory, VisibleHistory};
use re_log_types::{EntityPath, RowId};
use re_query::query_archetype;
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{AnyQuery, Caches};

// ---

/// Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]
/// (combined) for 1 point-of-view component and no optional components.
///
/// Alias for [`query_cached_archetype_pov1_comp0`].
#[inline]
pub fn query_cached_archetype_pov1<'a, A, R1, F>(
    store: &'a DataStore,
    query: &AnyQuery,
    entity_path: &'a EntityPath,
    f: F,
) -> ::re_query::Result<()>
where
    A: Archetype + 'a,
    R1: Component + Send + Sync + 'static,
    F: FnMut(&mut dyn Iterator<Item = (&(TimeInt, RowId), &[InstanceKey], &[R1])>),
{
    query_cached_archetype_pov1_comp0::<A, R1, F>(store, query, entity_path, f)
}

macro_rules! impl_query_cached_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) `" $N "` point-of-view components and `" $M "` optional components."]
        pub fn [<query_cached_archetype_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
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
                &mut dyn Iterator<
                    Item = (
                        &(TimeInt, RowId),
                        &[InstanceKey],
                        $(&[$pov],)+
                        $(&[Option<$comp>],)*
                    ),
                >,
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_cached_archetype_povN_compM",
                format!("arch={} pov={} comp={}", A::name(), $N, $M)
            );

            match &query {
                AnyQuery::LatestAtQuery(query) => {
                    Caches::with_latest_at::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |cache| {
                            re_tracing::profile_scope!("latest_at");

                             let bucket = cache.entry(query.at).or_default();
                            // NOTE: Implicitly dropping the write guard here: the LatestAtCache is
                            // free once again!

                            if bucket.is_empty() {
                                let now = web_time::Instant::now();
                                let arch_view = query_archetype::<A>(store, &query, entity_path)?;

                                bucket.[<insert_pov $N _comp$M>]::<A, $($pov,)+ $($comp,)*>(query.at, &arch_view)?;

                                // TODO(cmc): I'd love a way of putting this information into
                                // the `puffin` span directly.
                                let elapsed = now.elapsed();
                                ::re_log::trace!(
                                    "cached new entry in {elapsed:?} ({:0.3} entries/s)",
                                    1f64 / elapsed.as_secs_f64()
                                );
                            }

                            let mut it = itertools::izip!(
                                bucket.iter_pov_times(),
                                bucket.iter_pov_instance_keys(),
                                $(bucket.iter_component::<$pov>()?,)+
                                $(bucket.iter_component_opt::<$comp>()?,)*
                            );

                            f(&mut it);

                            Ok(())
                        }
                    )
                },
            }
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_cached_archetype!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_cached_archetype!(for N=1, M=NUM_COMP);
});

// ---

/// Cached implementation of [`re_query::query_archetype_with_history`] for 1 point-of-view component
/// and no optional components.
///
/// Alias for [`query_cached_archetype_with_history_pov1_comp0`].
#[inline]
pub fn query_cached_archetype_with_history_pov1<'a, A, R1, F>(
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
    F: FnMut(&mut dyn Iterator<Item = (&(TimeInt, RowId), &[InstanceKey], &[R1])>),
{
    query_cached_archetype_with_history_pov1_comp0::<A, R1, F>(
        store, timeline, time, history, ent_path, f,
    )
}

/// Generates a function to cache a (potentially historical) query with N point-of-view components and M
/// other components.
macro_rules! impl_query_cached_archetype_with_history {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype_with_history`] for `" $N "` point-of-view"]
        #[doc = "components and `" $M "` optional components."]
        pub fn [<query_cached_archetype_with_history_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
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
                &mut dyn Iterator<
                    Item = (
                        &(TimeInt, RowId),
                        &[InstanceKey],
                        $(&[$pov],)+
                        $(&[Option<$comp>],)*
                    ),
                >,
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_cached_archetype_with_history_povN_compM",
                format!("arch={} pov={} comp={}", A::name(), $N, $M)
            );

            let visible_history = match timeline.typ() {
                re_log_types::TimeType::Time => history.nanos,
                re_log_types::TimeType::Sequence => history.sequences,
            };

            if !history.enabled || visible_history == VisibleHistory::OFF {
                let query = LatestAtQuery::new(*timeline, *time);
                $crate::[<query_cached_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
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
            impl_query_cached_archetype_with_history!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_cached_archetype_with_history!(for N=1, M=NUM_COMP);
});
