use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_log_types::{EntityPath, RowId};
use re_query::{ExtraQueryHistory, VisibleHistory};
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

impl<'a, C> MaybeCachedComponentData<'a, Option<C>> {
    /// Iterates over the data of an optional component, or repeat `None` values if it's missing.
    #[inline]
    pub fn iter_or_repeat_opt(
        this: &Option<Self>,
        len: usize,
    ) -> impl Iterator<Item = &Option<C>> + '_ {
        this.as_ref().map_or(
            itertools::Either::Left(std::iter::repeat(&None).take(len)),
            |data| itertools::Either::Right(data.iter()),
        )
    }
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
/// Alias for [`Self::query_archetype_pov1_comp0`].
impl Caches {
    #[inline]
    pub fn query_archetype_pov1<'a, A, R1, F>(
        &self,
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
        self.query_archetype_pov1_comp0::<A, R1, F>(cached, store, query, entity_path, f)
    }
}

macro_rules! impl_query_archetype {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            &self,
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
                    $(Option<MaybeCachedComponentData<'_, Option<$comp>>>,)*
                ),
            ),
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!(
                "query_archetype",
                format!("cached={cached} arch={} pov={} comp={}", A::name(), $N, $M)
            );

            match &query {
                AnyQuery::LatestAt(query) if !cached => {
                    re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                    let arch_view = ::re_query::query_archetype::<A>(store, query, entity_path)?;

                    let data = (
                        (arch_view.data_time(), arch_view.primary_row_id()),
                        MaybeCachedComponentData::Raw(arch_view.iter_instance_keys().collect()),
                        $(MaybeCachedComponentData::Raw(arch_view.iter_required_component::<$pov>()?.collect()),)+
                        $(Some(MaybeCachedComponentData::Raw(arch_view.iter_optional_component::<$comp>()?.collect())),)*
                    );

                    f(data);

                    Ok(())
                }

                AnyQuery::LatestAt(query) => {
                    re_tracing::profile_scope!("latest_at", format!("{query:?}"));

                    self.[<query_archetype_latest_at_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* F>(
                        store,
                        query,
                        entity_path,
                        f,
                    )
                }

                AnyQuery::Range(query) if !cached => {
                    re_tracing::profile_scope!("range", format!("{query:?}"));

                    // NOTE: `+ 1` because we always grab the instance keys.
                    let arch_views = ::re_query::range_component_set::<A, { $N + $M + 1 }>(
                        store, query, entity_path,
                        &[$(<$pov>::name(),)+],
                        [<InstanceKey as re_types_core::Loggable>::name(), $(<$pov>::name(),)+ $(<$comp>::name(),)*],
                    );

                    for arch_view in arch_views {
                        let data = (
                            (arch_view.data_time(), arch_view.primary_row_id()),
                            MaybeCachedComponentData::Raw(arch_view.iter_instance_keys().collect()),
                            $(MaybeCachedComponentData::Raw(arch_view.iter_required_component::<$pov>()?.collect()),)+
                            $(Some(MaybeCachedComponentData::Raw(arch_view.iter_optional_component::<$comp>()?.collect())),)*
                        );

                        f(data);
                    }

                    Ok(())
                }

                AnyQuery::Range(query) => {
                    re_tracing::profile_scope!("range", format!("{query:?}"));

                    self.[<query_archetype_range_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* F>(
                        store,
                        query,
                        entity_path,
                        f,
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
        cached_latest_at: bool,
        cached_range: bool,
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
        self.query_archetype_with_history_pov1_comp0::<A, R1, F>(
            cached_latest_at,
            cached_range,
            store,
            timeline,
            time,
            history,
            ent_path,
            f,
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
            cached_latest_at: bool,
            cached_range: bool,
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
                    $(Option<MaybeCachedComponentData<'_, Option<$comp>>>,)*
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
                    format!("cached={cached_latest_at} arch={} pov={} comp={}", A::name(), $N, $M)
                );

                let query = LatestAtQuery::new(*timeline, *time);
                self.[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    cached_latest_at,
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            } else {
                // NOTE: not `profile_function!` because we want them merged together.
                re_tracing::profile_scope!(
                    "query_archetype_with_history",
                    format!("cached={cached_range} arch={} pov={} comp={}", A::name(), $N, $M)
                );

                let min_time = visible_history.from(*time);
                let max_time = visible_history.to(*time);
                let query = RangeQuery::new(*timeline, TimeRange::new(min_time, max_time));
                self.[<query_archetype_pov$N _comp$M>]::<A, $($pov,)+ $($comp,)* _>(
                    cached_range,
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
