use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
    sync::Arc,
};

use ahash::{HashMap, HashSet};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use paste::paste;
use seq_macro::seq;

use re_data_store::{
    LatestAtQuery, RangeQuery, StoreDiff, StoreEvent, StoreSubscriber, StoreSubscriberHandle,
};
use re_log_types::{EntityPath, RowId, StoreId, TimeInt, TimeRange, Timeline};
use re_query::ArchetypeView;
use re_types_core::{
    components::InstanceKey, Archetype, ArchetypeName, Component, ComponentName, SizeBytes as _,
};

use crate::{ErasedFlatVecDeque, FlatVecDeque, LatestAtCache, RangeCache};

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
static CACHES: Lazy<StoreSubscriberHandle> =
    Lazy::new(|| re_data_store::DataStore::register_subscriber(Box::<Caches>::default()));

/// Maintains the top-level cache mappings.
//
// TODO: since we have in cache key in there it means we're duplicating timeless data betweem each
// and every cache of an entity raaaaaaaaaaaaaaah
#[derive(Default)]
pub struct Caches(pub(crate) RwLock<HashMap<CacheKey, CachesPerArchetype>>);

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
    pub(crate) latest_at_per_archetype: RwLock<HashMap<ArchetypeName, Arc<RwLock<LatestAtCache>>>>,

    /// Which [`Archetype`] are we querying for?
    ///
    /// This is very important because of our data model: we not only query for components, but we
    /// query for components from a specific point-of-view (the so-called primary component).
    /// Different archetypes have different point-of-views, and therefore can end up with different
    /// results, even from the same raw data.
    //
    // TODO(cmc): At some point we should probably just store the PoV and optional components rather
    // than an `ArchetypeName`: the query system doesn't care about archetypes.
    pub(crate) range_per_archetype: RwLock<HashMap<ArchetypeName, Arc<RwLock<RangeCache>>>>,

    /// Everything greater than or equal to this timestamp has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching this criteria.
    /// `None` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pending_timeful_invalidation: Option<TimeInt>,

    /// If `true`, the timeless data associated with this cache has been asynchronously invalidated.
    ///
    /// If `true`, this cache must remove all of its timeless entries the next time it gets queried.
    /// `false` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pending_timeless_invalidation: bool,
}

impl Caches {
    /// Clears all caches.
    //
    // TODO(#4731): expose palette command.
    #[inline]
    pub fn clear() {
        re_data_store::DataStore::with_subscriber_once(*CACHES, |caches: &Caches| {
            caches.0.write().clear();
        });
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

        let cache =
            re_data_store::DataStore::with_subscriber_once(*CACHES, move |caches: &Caches| {
                let mut caches = caches.0.write();

                let caches_per_archetype = caches.entry(key.clone()).or_default();
                caches_per_archetype.handle_pending_invalidation(&key);

                let mut latest_at_per_archetype =
                    caches_per_archetype.latest_at_per_archetype.write();
                let latest_at_cache = latest_at_per_archetype.entry(A::name()).or_default();

                Arc::clone(latest_at_cache)

                // Implicitly releasing all intermediary locks.
            })
            // NOTE: downcasting cannot fail, this is our own private handle.
            .unwrap();

        let mut cache = cache.write();
        f(&mut cache)
    }

    /// Gives write access to the appropriate `RangeCache` according to the specified
    /// query parameters.
    #[inline]
    pub fn with_range<A, F, R>(
        store_id: StoreId,
        entity_path: EntityPath,
        query: &RangeQuery,
        mut f: F,
    ) -> R
    where
        A: Archetype,
        F: FnMut(&mut RangeCache) -> R,
    {
        let key = CacheKey::new(store_id, entity_path, query.timeline);

        let cache =
            re_data_store::DataStore::with_subscriber_once(*CACHES, move |caches: &Caches| {
                let mut caches = caches.0.write();

                let caches_per_archetype = caches.entry(key.clone()).or_default();
                caches_per_archetype.handle_pending_invalidation(&key);

                let mut range_per_archetype = caches_per_archetype.range_per_archetype.write();
                let range_cache = range_per_archetype.entry(A::name()).or_default();

                Arc::clone(range_cache)

                // Implicitly releasing all intermediary locks.
            })
            // NOTE: downcasting cannot fail, this is our own private handle.
            .unwrap();

        let mut cache = cache.write();
        f(&mut cache)
    }

    #[inline]
    pub(crate) fn with<F: FnMut(&Caches) -> R, R>(f: F) -> R {
        // NOTE: downcasting cannot fail, this is our own private handle.
        re_data_store::DataStore::with_subscriber(*CACHES, f).unwrap()
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

// --- Invalidation ---

impl StoreSubscriber for Caches {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscribers.QueryCache".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    // TODO(cmc): support dropped recordings.
    fn on_events(&mut self, events: &[StoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        for event in events {
            let StoreEvent {
                store_id,
                store_generation: _,
                event_id: _,
                diff,
            } = event;

            let StoreDiff {
                kind: _, // Don't care: both additions and deletions invalidate query results.
                row_id: _,
                times,
                entity_path,
                cells: _, // Don't care: we invalidate at the entity level, not component level.
            } = diff;

            #[derive(Default, Debug)]
            struct CompactedEvents {
                timeless: HashSet<(StoreId, EntityPath)>,
                timeful: HashMap<CacheKey, TimeInt>,
            }

            let mut compacted = CompactedEvents::default();
            {
                re_tracing::profile_scope!("compact events");

                if times.is_empty() {
                    compacted
                        .timeless
                        .insert((store_id.clone(), entity_path.clone()));
                }

                for &(timeline, time) in times {
                    let key = CacheKey::new(store_id.clone(), entity_path.clone(), timeline);
                    let min_time = compacted.timeful.entry(key).or_insert(TimeInt::MAX);
                    *min_time = TimeInt::min(*min_time, time);
                }
            }

            // TODO(cmc): This is horribly stupid and slow and can easily be made faster by adding
            // yet another layer of caching indirection.
            // But since this pretty much never happens in practice, let's not go there until we
            // have metrics showing that we need to.
            {
                re_tracing::profile_scope!("timeless");

                for (store_id, entity_path) in compacted.timeless {
                    for (key, caches_per_archetype) in self.0.write().iter_mut() {
                        if key.store_id == store_id && key.entity_path == entity_path {
                            caches_per_archetype.pending_timeless_invalidation = true;
                        }
                    }
                }
            }

            {
                re_tracing::profile_scope!("timeful");

                for (key, time) in compacted.timeful {
                    if let Some(caches_per_archetype) = self.0.write().get_mut(&key) {
                        if let Some(min_time) =
                            caches_per_archetype.pending_timeful_invalidation.as_mut()
                        {
                            *min_time = TimeInt::min(*min_time, time);
                        } else {
                            caches_per_archetype.pending_timeful_invalidation = Some(time);
                        }
                    }
                }
            }
        }
    }
}

impl CachesPerArchetype {
    /// Removes all entries from the cache that have been asynchronously invalidated.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    fn handle_pending_invalidation(&mut self, key: &CacheKey) {
        let pending_timeless_invalidation = self.pending_timeless_invalidation;
        let pending_timeful_invalidation = self.pending_timeful_invalidation.is_some();

        if !pending_timeless_invalidation && !pending_timeful_invalidation {
            return;
        }

        re_tracing::profile_function!();

        for latest_at_cache in self.latest_at_per_archetype.read().values() {
            let mut latest_at_cache = latest_at_cache.write();

            if pending_timeless_invalidation {
                latest_at_cache.timeless = None;
            }

            let mut removed_bytes = 0u64;
            if let Some(min_time) = self.pending_timeful_invalidation {
                latest_at_cache
                    .per_query_time
                    .retain(|&query_time, _| query_time < min_time);

                latest_at_cache.per_data_time.retain(|&data_time, bucket| {
                    if data_time < min_time {
                        return true;
                    }

                    // Only if that bucket is about to be dropped.
                    if Arc::strong_count(bucket) == 1 {
                        removed_bytes += bucket.read().total_size_bytes;
                    }

                    false
                });
            }

            latest_at_cache.total_size_bytes = latest_at_cache
                .total_size_bytes
                .checked_sub(removed_bytes)
                .unwrap_or_else(|| {
                    re_log::debug!(
                        store_id = %key.store_id,
                        entity_path = %key.entity_path,
                        current = latest_at_cache.total_size_bytes,
                        removed = removed_bytes,
                        "book keeping underflowed"
                    );
                    u64::MIN
                });
        }

        self.pending_timeful_invalidation = None;
        self.pending_timeless_invalidation = false;
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
    /// This corresponds to the data time and `RowId` returned by `re_query::query_archetype`.
    ///
    /// This is guaranteed to always be sorted and dense (i.e. there cannot be a hole in the cached
    /// data, unless the raw data itself in the store has a hole at that particular point in time).
    ///
    /// Reminder: within a single timestamp, rows are sorted according to their [`RowId`]s.
    pub(crate) data_times: VecDeque<(TimeInt, RowId)>,

    /// The [`InstanceKey`]s of the point-of-view components.
    pub(crate) pov_instance_keys: FlatVecDeque<InstanceKey>,

    /// The resulting component data, pre-deserialized, pre-joined.
    //
    // TODO(#4733): Don't denormalize auto-generated instance keys.
    // TODO(#4734): Don't denormalize splatted values.
    pub(crate) components: BTreeMap<ComponentName, Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    /// The total size in bytes stored in this bucket.
    ///
    /// Only used so we can decrement the global cache size when the last reference to a bucket
    /// gets dropped.
    pub(crate) total_size_bytes: u64,
    //
    // TODO(cmc): secondary cache
}

impl CacheBucket {
    #[inline]
    pub fn time_range(&self) -> Option<TimeRange> {
        let first_time = self.data_times.front().map(|(t, _)| *t)?;
        let last_time = self.data_times.back().map(|(t, _)| *t)?;
        Some(TimeRange::new(first_time, last_time))
    }

    #[inline]
    pub fn contains_data_time(&self, data_time: TimeInt) -> bool {
        let first_time = self.data_times.front().map_or(&TimeInt::MAX, |(t, _)| t);
        let last_time = self.data_times.back().map_or(&TimeInt::MIN, |(t, _)| t);
        *first_time <= data_time && data_time <= *last_time
    }

    #[inline]
    pub fn contains_data_row(&self, data_time: TimeInt, row_id: RowId) -> bool {
        self.data_times.binary_search(&(data_time, row_id)).is_ok()
    }

    /// How many timestamps' worth of data is stored in this bucket?
    #[inline]
    pub fn num_entries(&self) -> usize {
        self.data_times.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_entries() == 0
    }

    // ---

    /// Iterate over the timestamps of the point-of-view components.
    #[inline]
    pub fn iter_data_times(&self) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.data_times.iter()
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

    // ---

    /// Returns the index range that corresponds to the specified `time_range`.
    ///
    /// Use the returned range with one of the range iteration methods:
    /// - [`Self::range_data_times`]
    /// - [`Self::range_pov_instance_keys`]
    /// - [`Self::range_component`]
    /// - [`Self::range_component_opt`]
    ///
    /// Make sure that the bucket hasn't been modified in-between!
    ///
    /// This is `O(2*log(n))`, so make sure to clone the returned range rather than calling this
    /// multiple times.
    #[inline]
    pub fn entry_range(&self, time_range: TimeRange) -> Range<usize> {
        let start_index = self
            .data_times
            .partition_point(|t| t < &(time_range.min, RowId::ZERO));
        let end_index = self
            .data_times
            .partition_point(|t| t < &(time_range.max, RowId::MAX));
        start_index..end_index
    }

    /// Range over the timestamps of the point-of-view components.
    #[inline]
    pub fn range_data_times(
        &self,
        entry_range: Range<usize>,
    ) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.data_times.range(entry_range)
    }

    /// Range over the [`InstanceKey`] batches of the point-of-view components.
    #[inline]
    pub fn range_pov_instance_keys(
        &self,
        entry_range: Range<usize>,
    ) -> impl Iterator<Item = &[InstanceKey]> {
        self.pov_instance_keys.range(entry_range)
    }

    /// Range over the batches of the specified non-optional component.
    #[inline]
    pub fn range_component<C: Component + Send + Sync + 'static>(
        &self,
        entry_range: Range<usize>,
    ) -> Option<impl Iterator<Item = &[C]>> {
        let data = self
            .components
            .get(&C::name())
            .and_then(|data| data.as_any().downcast_ref::<FlatVecDeque<C>>())?;
        Some(data.range(entry_range))
    }

    /// Range over the batches of the specified optional component.
    #[inline]
    pub fn range_component_opt<C: Component + Send + Sync + 'static>(
        &self,
        entry_range: Range<usize>,
    ) -> Option<impl Iterator<Item = &[Option<C>]>> {
        let data = self
            .components
            .get(&C::name())
            .and_then(|data| data.as_any().downcast_ref::<FlatVecDeque<Option<C>>>())?;
        Some(data.range(entry_range))
    }
}

macro_rules! impl_insert {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Inserts the contents of the given [`ArchetypeView`], which are made of the specified"]
        #[doc = "`" $N "` point-of-view components and `" $M "` optional components, to the cache."]
        #[doc = ""]
        #[doc = "Returns the size in bytes of the data that was cached."]
        #[doc = ""]
        #[doc = "`query_time` must be the time of query, _not_ of the resulting data."]
        pub fn [<insert_pov$N _comp$M>]<A, $($pov,)+ $($comp),*>(
            &mut self,
            query_time: TimeInt,
            arch_view: &ArchetypeView<A>,
        ) -> ::re_query::Result<u64>
        where
            A: Archetype,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
        {
            // NOTE: not `profile_function!` because we want them merged together.
            re_tracing::profile_scope!("CacheBucket::insert", format!("arch={} pov={} comp={}", A::name(), $N, $M));

            let pov_row_id = arch_view.primary_row_id();
            let index = self.data_times.partition_point(|t| t < &(query_time, pov_row_id));

            let mut added_size_bytes = 0u64;

            self.data_times.insert(index, (query_time, pov_row_id));
            added_size_bytes += (query_time, pov_row_id).total_size_bytes();

            {
                // The `FlatVecDeque` will have to collect the data one way or another: do it ourselves
                // instead, that way we can efficiently computes its size while we're at it.
                let added: FlatVecDeque<InstanceKey> = arch_view
                    .iter_instance_keys()
                    .collect::<VecDeque<InstanceKey>>()
                    .into();
                added_size_bytes += added.total_size_bytes();
                self.pov_instance_keys.insert_deque(index, added);
            }

            $(added_size_bytes += self.insert_component::<A, $pov>(index, arch_view)?;)+
            $(added_size_bytes += self.insert_component_opt::<A, $comp>(index, arch_view)?;)*

            self.total_size_bytes += added_size_bytes;

            Ok(added_size_bytes)
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
    ) -> ::re_query::Result<u64>
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
    ) -> re_query::Result<u64> {
        re_tracing::profile_function!(C::name());

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<C>::new()));

        // The `FlatVecDeque` will have to collect the data one way or another: do it ourselves
        // instead, that way we can efficiently computes its size while we're at it.
        let added: FlatVecDeque<C> = arch_view
            .iter_required_component::<C>()?
            .collect::<VecDeque<C>>()
            .into();
        let added_size_bytes = added.total_size_bytes();

        // NOTE: downcast cannot fail, we create it just above.
        let data = data.as_any_mut().downcast_mut::<FlatVecDeque<C>>().unwrap();
        data.insert_deque(at, added);

        Ok(added_size_bytes)
    }

    #[inline]
    fn insert_component_opt<A: Archetype, C: Component + Send + Sync + 'static>(
        &mut self,
        at: usize,
        arch_view: &ArchetypeView<A>,
    ) -> re_query::Result<u64> {
        re_tracing::profile_function!(C::name());

        let data = self
            .components
            .entry(C::name())
            .or_insert_with(|| Box::new(FlatVecDeque::<Option<C>>::new()));

        // The `FlatVecDeque` will have to collect the data one way or another: do it ourselves
        // instead, that way we can efficiently computes its size while we're at it.
        let added: FlatVecDeque<Option<C>> = arch_view
            .iter_optional_component::<C>()?
            .collect::<VecDeque<Option<C>>>()
            .into();
        let added_size_bytes = added.total_size_bytes();

        // NOTE: downcast cannot fail, we create it just above.
        let data = data
            .as_any_mut()
            .downcast_mut::<FlatVecDeque<Option<C>>>()
            .unwrap();
        data.insert_deque(at, added);

        Ok(added_size_bytes)
    }
}
