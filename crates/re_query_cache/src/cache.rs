use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use ahash::HashMap;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use paste::paste;
use seq_macro::seq;

use re_data_store::{LatestAtQuery, RangeQuery};
use re_log_types::{EntityPath, RowId, StoreId, TimeInt, Timeline};
use re_query::ArchetypeView;
use re_types_core::{components::InstanceKey, Archetype, ArchetypeName, Component, ComponentName};

use crate::{ErasedFlatVecDeque, FlatVecDeque};

// ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnyQuery {
    LatestAt(LatestAtQuery),
    Range(RangeQuery),
}

impl From<LatestAtQuery> for AnyQuery {
    #[inline]
    fn from(query: LatestAtQuery) -> Self {
        Self::LatestAt(query)
    }
}

impl From<RangeQuery> for AnyQuery {
    #[inline]
    fn from(query: RangeQuery) -> Self {
        Self::Range(query)
    }
}

// ---

/// All primary caches (all stores, all entities, everything).
//
// TODO(cmc): Centralize and harmonize all caches (query, jpeg, mesh).
static CACHES: Lazy<Caches> = Lazy::new(Caches::default);

/// Maintains the top-level cache mappings.
//
// TODO(cmc): Store subscriber and cache invalidation.
// TODO(#4730): SizeBytes support + size stats + mem panel
#[derive(Default)]
pub struct Caches(RwLock<HashMap<CacheKey, CachesPerArchetype>>);

#[derive(Default)]
pub struct CachesPerArchetype {
    /// Which [`Archetype`] are we querying for?
    ///
    /// This is very important because of our data model: we not only query for components, but we
    /// query for components from a specific point-of-view (the so-called primary component).
    /// Different archetypes have different point-of-views, and therefore can end up with different
    /// results, even from the same raw data.
    //
    // TODO(cmc): At some point we should probably just store the PoV and optional components rather
    // than an `ArchetypeName`: the query system doesn't care about archetypes.
    latest_at_per_archetype: RwLock<HashMap<ArchetypeName, Arc<RwLock<LatestAtCache>>>>,
}

impl Caches {
    /// Clears all caches.
    //
    // TODO(#4731): expose palette command.
    #[inline]
    pub fn clear() {
        let Caches(caches) = &*CACHES;
        caches.write().clear();
    }

    /// Gives write access to the appropriate `LatestAtCache` according to the specified
    /// query parameters.
    #[inline]
    pub fn with_latest_at<A, F, R>(
        store_id: StoreId,
        entity_path: EntityPath,
        query: &LatestAtQuery,
        mut f: F,
    ) -> R
    where
        A: Archetype,
        F: FnMut(&mut LatestAtCache) -> R,
    {
        let key = CacheKey::new(store_id, entity_path, query.timeline);

        // We want to make sure we release the lock on the top-level cache map ASAP.
        let cache = {
            let mut caches = CACHES.0.write();
            let caches_per_archetype = caches.entry(key).or_default();
            let mut latest_at_per_archetype = caches_per_archetype.latest_at_per_archetype.write();
            let latest_at_cache = latest_at_per_archetype.entry(A::name()).or_default();
            Arc::clone(latest_at_cache)
        };

        let mut cache = cache.write();
        f(&mut cache)
    }
}

/// Uniquely identifies cached query results in the [`Caches`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey {
    /// Which [`re_data_store::DataStore`] is the query targeting?
    pub store_id: StoreId,

    /// Which [`EntityPath`] is the query targeting?
    pub entity_path: EntityPath,

    /// Which [`Timeline`] is the query targeting?
    pub timeline: Timeline,
}

impl CacheKey {
    #[inline]
    pub fn new(
        store_id: impl Into<StoreId>,
        entity_path: impl Into<EntityPath>,
        timeline: impl Into<Timeline>,
    ) -> Self {
        Self {
            store_id: store_id.into(),
            entity_path: entity_path.into(),
            timeline: timeline.into(),
        }
    }
}

// ---

/// Caches the results of any query for an arbitrary range of time.
///
/// This caches all the steps involved in getting data ready for space views:
/// - index search,
/// - instance key joining,
/// - deserialization.
///
/// We share the `CacheBucket` implementation between all types of queries to avoid duplication of
/// logic, especially for things that require metaprogramming, to keep the macro madness to a
/// minimum.
/// In the case of `LatestAt` queries, a `CacheBucket` will always contain a single timestamp worth
/// of data.
#[derive(Default)]
pub struct CacheBucket {
    /// The _data_ timestamps and [`RowId`]s of all cached rows.
    ///
    /// Reminder: within a single timestamp, rows are sorted according to their [`RowId`]s.
    pub(crate) pov_data_times: VecDeque<(TimeInt, RowId)>,

    /// The [`InstanceKey`]s of the point-of-view components.
    pub(crate) pov_instance_keys: FlatVecDeque<InstanceKey>,

    /// The resulting component data, pre-deserialized, pre-joined.
    //
    // TODO(#4733): Don't denormalize auto-generated instance keys.
    // TODO(#4734): Don't denormalize splatted values.
    pub(crate) components: BTreeMap<ComponentName, Box<dyn ErasedFlatVecDeque + Send + Sync>>,
    //
    // TODO(cmc): secondary cache
    // TODO(#4730): size stats: this requires codegen'ing SizeBytes for all components!
}

impl CacheBucket {
    /// Iterate over the timestamps of the point-of-view components.
    #[inline]
    pub fn iter_pov_data_times(&self) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.pov_data_times.iter()
    }

    /// Iterate over the [`InstanceKey`] batches of the point-of-view components.
    #[inline]
    pub fn iter_pov_instance_keys(&self) -> impl Iterator<Item = &[InstanceKey]> {
        self.pov_instance_keys.iter()
    }

    /// Iterate over the batches of the specified non-optional component.
    #[inline]
    pub fn iter_component<C: Component + Send + Sync + 'static>(
        &self,
    ) -> Option<impl Iterator<Item = &[C]>> {
        let data = self
            .components
            .get(&C::name())
            .and_then(|data| data.as_any().downcast_ref::<FlatVecDeque<C>>())?;
        Some(data.iter())
    }

    /// Iterate over the batches of the specified optional component.
    #[inline]
    pub fn iter_component_opt<C: Component + Send + Sync + 'static>(
        &self,
    ) -> Option<impl Iterator<Item = &[Option<C>]>> {
        let data = self
            .components
            .get(&C::name())
            .and_then(|data| data.as_any().downcast_ref::<FlatVecDeque<Option<C>>>())?;
        Some(data.iter())
    }

    /// How many timestamps' worth of data is stored in this bucket?
    #[inline]
    pub fn num_entries(&self) -> usize {
        self.pov_data_times.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_entries() == 0
    }
}

macro_rules! impl_insert {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Inserts the contents of the given [`ArchetypeView`], which are made of the specified"]
        #[doc = "`" $N "` point-of-view components and `" $M "` optional components, to the cache."]
        #[doc = ""]
        #[doc = "`query_time` must be the time of query, _not_ of the resulting data."]
        pub fn [<insert_pov$N _comp$M>]<A, $($pov,)+ $($comp),*>(
            &mut self,
            query_time: TimeInt,
            arch_view: &ArchetypeView<A>,
        ) -> ::re_query::Result<()>
        where
            A: Archetype,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!("CacheBucket::insert", format!("arch={} pov={} comp={}", A::name(), $N, $M));

            let Self {
                pov_data_times,
                pov_instance_keys,
                components: _,
            } = self;

            let pov_row_id = arch_view.primary_row_id();
            let index = pov_data_times.partition_point(|t| t < &(query_time, pov_row_id));

            pov_data_times.insert(index, (query_time, pov_row_id));
            pov_instance_keys.insert(index, arch_view.iter_instance_keys());
            $(self.insert_component::<A, $pov>(index, arch_view)?;)+
            $(self.insert_component_opt::<A, $comp>(index, arch_view)?;)*

            Ok(())
        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_insert!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

impl CacheBucket {
    /// Alias for [`Self::insert_pov1_comp0`].
    #[inline]
    #[allow(dead_code)]
    fn insert_pov1<A, R1>(
        &mut self,
        query_time: TimeInt,
        arch_view: &ArchetypeView<A>,
    ) -> ::re_query::Result<()>
    where
        A: Archetype,
        R1: Component + Send + Sync + 'static,
    {
        self.insert_pov1_comp0::<A, R1>(query_time, arch_view)
    }

    seq!(NUM_COMP in 0..10 {
        impl_insert!(for N=1, M=NUM_COMP);
    });

    #[inline]
    fn insert_component<A: Archetype, C: Component + Send + Sync + 'static>(
        &mut self,
        at: usize,
        arch_view: &ArchetypeView<A>,
    ) -> re_query::Result<()> {
        re_tracing::profile_function!(C::name());

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<C>::new()));

        // NOTE: downcast cannot fail, we create it just above.
        let data = data.as_any_mut().downcast_mut::<FlatVecDeque<C>>().unwrap();
        data.insert(at, arch_view.iter_required_component::<C>()?);

        Ok(())
    }

    #[inline]
    fn insert_component_opt<A: Archetype, C: Component + Send + Sync + 'static>(
        &mut self,
        at: usize,
        arch_view: &ArchetypeView<A>,
    ) -> re_query::Result<()> {
        re_tracing::profile_function!(C::name());

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<Option<C>>::new()));

        // NOTE: downcast cannot fail, we create it just above.
        let data = data
            .as_any_mut()
            .downcast_mut::<FlatVecDeque<Option<C>>>()
            .unwrap();
        data.insert(at, arch_view.iter_optional_component::<C>()?);

        Ok(())
    }
}

// ---

// NOTE: Because we're working with deserialized data, everything has to be done with metaprogramming,
// which is notoriously painful in Rust (i.e., macros).
// For this reason we move as much of the code as possible into the already existing macros in `query.rs`.

/// Caches the results of `LatestAt` archetype queries (`ArchetypeView`).
///
/// There is one `LatestAtCache` for each unique [`CacheKey`].
///
/// All query steps are cached: index search, cluster key joins and deserialization.
#[derive(Default)]
pub struct LatestAtCache {
    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query (i.e. run the
    /// index search in order to find a data time, but don't actually deserialize and join the data)
    /// and check if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
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
}
