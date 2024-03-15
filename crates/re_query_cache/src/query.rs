use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, Timeline};
use re_log_types::{EntityPath, RowId};
use re_query::{ExtraQueryHistory, VisibleHistory};
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{AnyQuery, Caches};

// ---

/// Iterates over the data of an optional component, or repeat `None` values if it's missing.
#[inline]
pub fn iter_or_repeat_opt<C>(
    this: Option<&[Option<C>]>,
    len: usize,
) -> impl Iterator<Item = &Option<C>> + '_ {
    this.as_ref().map_or(
        itertools::Either::Left(std::iter::repeat(&None).take(len)),
        |data| itertools::Either::Right(data.iter()),
    )
}

// ---

/// Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]
/// (combined) for 1 point-of-view component and no optional components.
///
/// Alias for [`Self::query_archetype_pov1_comp0`].
impl Caches {
    #[inline]
    pub fn query_archetype_pov1<'a, A, R1, F>(
        &self,
        store: &'a DataStore,
        query: &AnyQuery,
        entity_path: &'a EntityPath,
        f: F,
    ) -> ::re_query::Result<()>
    where
        A: Archetype + 'a,
        R1: Component + Send + Sync + 'static,
        F: FnMut(((TimeInt, RowId), &[InstanceKey], &[R1])),
    {
        self.query_archetype_pov1_comp0::<A, R1, F>(store, query, entity_path, f)
    }
}

macro_rules! impl_query_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            &self,
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
                    &[InstanceKey],
                    $(&[$pov],)+
                    $(Option<&[Option<$comp>]>,)*
                ),
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype",
                format!("cached=true arch={} pov={} comp={}", A::name(), $N, $M)
            );

            match &query {
                AnyQuery::LatestAt(query) => {
                    re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                    self.[<query_archetype_latest_at_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* F>(
                        store,
                        query,
                        entity_path,
                        f,
                    )
                }

                AnyQuery::Range(query) => {
                    re_tracing::profile_scope!("range", format!("{query:?}"));

                    self.[<query_archetype_range_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                        store,
                        query,
                        entity_path,
                        |entry_range, (data_times, pov_instance_keys, $($pov,)+ $($comp,)*)| {
                            let it = itertools::izip!(
                                data_times.range(entry_range.clone()),
                                pov_instance_keys.range(entry_range.clone()),
                                $($pov.range(entry_range.clone()),)+
                                $($comp.map_or_else(
                                    || itertools::Either::Left(std::iter::repeat(&[] as &[Option<$comp>])),
                                    |data| itertools::Either::Right(data.range(entry_range.clone())))
                                ,)*
                            ).map(|((time, row_id), instance_keys, $($pov,)+ $($comp,)*)| {
                                (
                                    (*time, *row_id),
                                    instance_keys,
                                    $($pov,)+
                                    $((!$comp.is_empty()).then_some($comp),)*
                                )
                            });

                            for data in it {
                                f(data);
                            }
                        },
                    )
                }
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

impl Caches {
    seq!(NUM_COMP in 0..10 {
        impl_query_archetype!(for N=1, M=NUM_COMP);
    });
}

// ---

/// Cached implementation of [`re_query::query_archetype_with_history`] for 1 point-of-view component
/// and no optional components.
///
/// Alias for [`Self::query_archetype_with_history_pov1_comp0`].
impl Caches {
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn query_archetype_with_history_pov1<'a, A, R1, F>(
        &self,
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
        F: FnMut(((TimeInt, RowId), &[InstanceKey], &[R1])),
    {
        self.query_archetype_with_history_pov1_comp0::<A, R1, F>(
            store, timeline, time, history, ent_path, f,
        )
    }
}

/// Generates a function to cache a (potentially historical) query with N point-of-view components and M
/// other components.
macro_rules! impl_query_archetype_with_history {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype_with_history`] for `" $N "` point-of-view"]
        #[doc = "components and `" $M "` optional components."]
        #[allow(clippy::too_many_arguments)]
        pub fn [<query_archetype_with_history_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            &self,
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
                    &[InstanceKey],
                    $(&[$pov],)+
                    $(Option<&[Option<$comp>]>,)*
                ),
            ),
        {

            let visible_history = match timeline.typ() {
                re_log_types::TimeType::Time => history.nanos,
                re_log_types::TimeType::Sequence => history.sequences,
            };

            if !history.enabled || visible_history == VisibleHistory::OFF {
                // NOTE: not `profile_function!` because we want them merged together.
                re_tracing::profile_scope!(
                    "query_archetype_with_history",
                    format!("cached=true arch={} pov={} comp={}", A::name(), $N, $M)
                );

                let query = LatestAtQuery::new(*timeline, *time);
                self.[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            } else {
                // NOTE: not `profile_function!` because we want them merged together.
                re_tracing::profile_scope!(
                    "query_archetype_with_history",
                    format!("cached=true arch={} pov={} comp={}", A::name(), $N, $M)
                );

                let query = RangeQuery::new(*timeline, visible_history.time_range(*time));
                self.[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
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

impl Caches {
    seq!(NUM_COMP in 0..10 {
        impl_query_archetype_with_history!(for N=1, M=NUM_COMP);
    });
}
