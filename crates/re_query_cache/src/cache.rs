use std::{
    collections::{BTreeMap, VecDeque},
    ops::{Range, RangeInclusive},
    sync::{atomic::AtomicBool, Arc},
};

use ahash::{HashMap, HashSet};
use itertools::{Either, Itertools};
use nohash_hasher::{IntMap, IntSet};
use once_cell::sync::Lazy;
use parking_lot::{RwLock, RwLockWriteGuard};

use re_arrow_store::{
    LatestAtQuery, RangeQuery, StoreDiff, StoreDiffKind, StoreEvent, StoreSubscriber,
    StoreSubscriberHandle, TimeRange,
};
use re_log_types::{EntityPath, RowId, StoreId, TimeInt, Timeline, VecDequeRemovalExt as _};
use re_query::ArchetypeView;
use re_types_core::{
    components::InstanceKey, Archetype, ArchetypeName, Component, ComponentName, ComponentNameSet,
    SizeBytes,
};

use crate::{ErasedFlatVecDeque, FlatVecDeque};

// ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnyQuery {
    // TODO: range support
    LatestAtQuery(LatestAtQuery),
}

impl From<LatestAtQuery> for AnyQuery {
    fn from(query: LatestAtQuery) -> Self {
        Self::LatestAtQuery(query)
    }
}

// ---

// TODO: harmnonize all caches.
// TODO: this should be in the `Caches` thing in ViewContext, I think>
pub static CACHES: Lazy<Caches> = Lazy::new(Caches::default);

// TODO: need a timeless version for both of those (use special name?)
// TODO: this should be in the `Caches` thing in ViewContext, I think>
// TODO: create issue to unify all queries behind a single configurable one
#[derive(Default)]
pub struct Caches {
    latest_at: RwLock<HashMap<CacheKey, Arc<RwLock<LatestAtCache>>>>,
}

impl Caches {
    fn with_global<F: FnMut(&Caches) -> R, R>(mut f: F) -> R {
        f(&CACHES)
    }

    #[inline]
    pub fn with_latest_at<A, F, R>(
        store_id: StoreId,
        entity_path: EntityPath,
        query: &LatestAtQuery,
        mut f: F,
    ) -> R
    where
        A: Archetype,
        F: FnMut(RwLockWriteGuard<'_, LatestAtCache>) -> R,
    {
        // TODO: having to clone params is dumb...
        Self::with_global(move |caches| {
            let key = CacheKey::new(
                store_id.clone(),
                entity_path.clone(),
                query.timeline,
                A::name(),
            );

            let cache = {
                let mut caches = caches.latest_at.write();
                let cache = caches.entry(key).or_default();
                Arc::clone(cache)
            };

            f(cache.write())
        })
    }
}

/// Identifies cached query results in the `[Caches]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey {
    /// Which [`DataStore`] is the query running on?
    pub store_id: StoreId,

    /// Which [`EntityPath`] is the query running on?
    pub entity_path: EntityPath,

    /// Which [`Timeline`] is the query running on?
    pub timeline: Timeline,

    /// Which [`Archetype`] are we querying for?
    ///
    /// This is very important because of our data model: we not only query for components, but we
    /// query for components from a specific point-of-view (the so-called primary component).
    /// Different archetypes have different point-of-views, and therefore can end up with different
    /// results, even from the same raw data.
    pub archetype_name: ArchetypeName,
    //
    // TODO: maybe remove archetype and replace by PoV + components instead?
    // pub required_components: IntSet<ComponentName>,

    // TODO: support multiple point-of-views.
    // pub pov: ComponentName,
    //
    // pub components: ComponentNameSet,
}

impl CacheKey {
    #[inline]
    pub fn new(
        store_id: impl Into<StoreId>,
        entity_path: impl Into<EntityPath>,
        timeline: impl Into<Timeline>,
        archetype_name: impl Into<ArchetypeName>,
    ) -> Self {
        Self {
            store_id: store_id.into(),
            entity_path: entity_path.into(),
            timeline: timeline.into(),
            archetype_name: archetype_name.into(),
        }
    }
}

// ---

// TODO: this only implements the non-metaprogramming stuff, everything else gets inlines in
// query.rs to avoid a combinatorial explosion of macros.

/// Caches the results of `LatestAt` queries.
//
// TODO: document what this TimeInt identifies: the time of the query, _not_ the time of the data!
// btw, dropping data cancels ALL queries beyond that data, right?
#[derive(Default)]
pub struct LatestAtCache(BTreeMap<TimeInt, CacheBucket>);

impl std::ops::Deref for LatestAtCache {
    type Target = BTreeMap<TimeInt, CacheBucket>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for LatestAtCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl LatestAtCache {
    // TODO: it's query time rather than data time so these shouldnt even be used.
    // pub fn bucket_time(&self, query_time: TimeInt) -> Option<TimeInt> {
    //     let mut buckets = self.range(..=query_time).rev();
    //     buckets.next().map(|(time, _)| *time)
    // }
    //
    // pub fn next_bucket_time(&self, query_time: TimeInt) -> Option<TimeInt> {
    //     let mut buckets = self.range(TimeInt::from(query_time.as_i64().saturating_add(1))..);
    //     buckets.next().map(|(time, _)| *time)
    // }
}

// ---

// TODO: let's start by not invalidating anything
#[derive(Default)]
pub struct CacheBucket {
    /// The timestamps and [`RowId`]s of all cached rows.
    ///
    /// Reminder: within a single timestamp, rows are sorted according to their [`RowId`]s.
    //
    // TODO: pov_times
    pub(crate) times: VecDeque<(TimeInt, RowId)>,

    /// The [`InstanceKey`]s of the components TODO
    //
    // TODO: pov_instance_keys
    pub(crate) instance_keys: FlatVecDeque<InstanceKey>,

    /// The resulting component data, pre-deserialized, pre-joined.
    //
    // TODO: pre-deserialized, pre-joined
    // TODO: maybe in some cases we want to keep it arrow all the way...
    // TODO: intmap??
    pub(crate) components: BTreeMap<ComponentName, Box<dyn ErasedFlatVecDeque + Send + Sync>>,
    //
    // TODO(cmc): secondary cache
    // TODO(cmc): size stats
}

// TODO: doc
macro_rules! impl_add_povN_compM {
    (impl $name:ident with povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => {
        // TODO: everything that does not require template metaprog needs to go out of here
        pub fn $name<A, $($pov,)+ $($comp),*>(&mut self, time: TimeInt, arch_view: &ArchetypeView<A>)
        where
            A: Archetype,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
        {
            re_tracing::profile_scope!("CacheBucket::add_povN_compM", A::name());

            let Self {
                times,
                instance_keys,
                components: _,
            } = self;

            let row_id = arch_view.primary_row_id();

            // TODO: always sorted as long as we don't introduce OOO inserts
            // TODO: we can have many entries per time!!!!!!!!

            let index = match times.binary_search(&(time, row_id)) {
                Ok(index) => index,
                Err(index) => {
                    if index == 0 || index == times.len() {
                        index
                    } else {
                        dbg!(index) // TODO
                        // // TODO: what does OOO actually mean in this instance??
                        // dbg!((index, times.len(), time, times.back()));
                        // unimplemented!("OOO inserts")
                    }
                }
            };

            times.insert(index, (time, row_id));
            times.make_contiguous(); // TODO: that's a no

            instance_keys.insert(index, arch_view.iter_instance_keys());

            $(self.add_component_at::<A, $pov>(index, arch_view);)+

            $(self.add_component_opt_at::<A, $comp>(index, arch_view);)*
        }
    };
    (impl $name:ident with povs=[$($pov:ident)+]) => {
        impl_add_povN_compM!(impl $name with povs=[$($pov)+] comps=[]);
    };
}

impl CacheBucket {
    impl_add_povN_compM!(impl add_pov1        with povs=[R1]);
    impl_add_povN_compM!(impl add_pov1_comp1 with povs=[R1] comps=[O1]);
    impl_add_povN_compM!(impl add_pov1_comp2 with povs=[R1] comps=[O1 O2]);
    impl_add_povN_compM!(impl add_pov1_comp3 with povs=[R1] comps=[O1 O2 O3]);
    impl_add_povN_compM!(impl add_pov1_comp4 with povs=[R1] comps=[O1 O2 O3 O4]);
    impl_add_povN_compM!(impl add_pov1_comp5 with povs=[R1] comps=[O1 O2 O3 O4 O5]);
    impl_add_povN_compM!(impl add_pov1_comp6 with povs=[R1] comps=[O1 O2 O3 O4 O5 O6]);
    impl_add_povN_compM!(impl add_pov1_comp7 with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7]);
    impl_add_povN_compM!(impl add_pov1_comp8 with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7 O8]);
    impl_add_povN_compM!(impl add_pov1_comp9 with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7 O8 O9]);

    #[inline]
    fn add_component_at<A: Archetype, C: Component + Send + Sync + 'static>(
        &mut self,
        at: usize,
        arch_view: &ArchetypeView<A>,
    ) {
        re_tracing::profile_function!();

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<C>::new()));

        let data = data.as_any_mut().downcast_mut::<FlatVecDeque<C>>().unwrap(); // TODO
        data.insert(at, arch_view.iter_required_component::<C>().unwrap()); // TODO

        // TODO: oh shit we need all components to implement SizeBytes??!
        // let total_size_bytes = self
        //     .components_total_size_bytes
        //     .entry(C::name())
        //     .or_default();
        // *total_size_bytes = data.total_size_bytes();
    }

    #[inline]
    fn add_component_opt_at<A: Archetype, C: Component + Send + Sync + 'static>(
        &mut self,
        at: usize,
        arch_view: &ArchetypeView<A>,
    ) {
        re_tracing::profile_function!();

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<Option<C>>::new()));

        let data = data
            .as_any_mut()
            .downcast_mut::<FlatVecDeque<Option<C>>>()
            .unwrap(); // TODO
        data.insert(at, arch_view.iter_optional_component::<C>().unwrap()); // TODO
    }

    // TODO: iter_pov_xxx
    #[inline]
    pub fn iter_times(&self) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.times.iter()
    }

    // TODO: iter_pov_xxx
    #[inline]
    pub fn iter_instance_keys(&self) -> impl Iterator<Item = &[InstanceKey]> {
        self.instance_keys.iter()
    }

    #[inline]
    pub fn iter_component<C: Component + Send + Sync + 'static>(
        &self,
    ) -> impl Iterator<Item = &[C]> {
        let data = self
            .components
            .get(&C::name())
            .map(|data| data.as_any().downcast_ref::<FlatVecDeque<C>>().unwrap()); // TODO

        let Some(data) = data else {
            return Either::Left(std::iter::empty());
        };

        Either::Right(data.iter())
    }

    #[inline]
    pub fn iter_component_opt<C: Component + Send + Sync + 'static>(
        &self,
    ) -> impl Iterator<Item = &[Option<C>]> {
        let data = self.components.get(&C::name()).map(|data| {
            data.as_any()
                .downcast_ref::<FlatVecDeque<Option<C>>>()
                .unwrap()
        }); // TODO

        let Some(data) = data else {
            return Either::Left(std::iter::empty());
        };

        Either::Right(data.iter())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---

// TODO: experiments, remove

enum InstanceKeys {
    AutoGenerated(u32),
    Keys(Vec<InstanceKey>),
}

struct ComponentView {
    row_id: RowId,
    instance_keys: InstanceKeys,
    values: Box<dyn ErasedFlatVecDeque>,
}

struct QueryView {
    // TODO: support multiple point-of-views.
    pov: ComponentView,
    components: BTreeMap<ComponentName, ComponentView>,
}
