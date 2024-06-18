use std::collections::BTreeSet;
use std::ops::Range;
use std::{collections::BTreeMap, sync::Arc};

use arrow2::Either;
use indexmap::IndexMap;
use itertools::{izip, Itertools};
use parking_lot::RwLock;

use re_chunk::{Chunk, ChunkId, ChunkTimeline, RangeQuery, RowId, TimePoint, Timeline};
use re_chunk_store::{ChunkStore, LatestAtQuery, TimeInt};
use re_log_types::EntityPath;
use re_types_core::{components::ClearIsRecursive, ComponentName, Loggable as _, SizeBytes};

use crate::{CacheKey, Caches};

// TODO: "bucket" terminology makes no sense anymore

// ---

/// Compute the ordering of two data indices, making sure to deal with `STATIC` data appropriately.
//
// TODO(cmc): Maybe at some point we'll want to introduce a dedicated `DataIndex` type with
// proper ordering operators etc.
// It's harder than it sounds though -- depending on the context, you don't necessarily want index
// ordering to behave the same way.
fn compare_indices(lhs: (TimeInt, RowId), rhs: (TimeInt, RowId)) -> std::cmp::Ordering {
    match (lhs, rhs) {
        ((TimeInt::STATIC, lhs_row_id), (TimeInt::STATIC, rhs_row_id)) => {
            lhs_row_id.cmp(&rhs_row_id)
        }
        ((_, _), (TimeInt::STATIC, _)) => std::cmp::Ordering::Less,
        ((TimeInt::STATIC, _), (_, _)) => std::cmp::Ordering::Greater,
        _ => lhs.cmp(&rhs),
    }
}

impl Caches {
    // TODO: right now the only (good) reason to query N components at once is that it amortizes
    // the cost of computing clears, which is a bit meh.
    //
    /// Queries for the given `component_names` using latest-at semantics.
    ///
    /// See [`LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at2(
        &self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> QueryResults {
        re_tracing::profile_function!(entity_path.to_string());

        // Query-time clears
        // -----------------
        //
        // We need to find, at query time, whether there exist a `Clear` component that should
        // shadow part or all of the results that we are about to return.
        //
        // This is a two-step process.
        //
        // First, we need to find all `Clear` components that could potentially affect the returned
        // results, i.e. any `Clear` component on the entity itself, or any recursive `Clear`
        // component on any of its recursive parents.
        //
        // Then, we need to compare the index of each component result with the index of the most
        // recent relevant `Clear` component that was found: if there exists a `Clear` component with
        // both a _data time_ lesser or equal to the _query time_ and an index greater or equal
        // than the indexed of the returned data, then we know for sure that the `Clear` shadows
        // the data.
        let mut max_clear_index = (TimeInt::MIN, RowId::ZERO);
        {
            re_tracing::profile_scope!("clears");

            let mut clear_entity_path = entity_path.clone();
            loop {
                let key = CacheKey::new(
                    clear_entity_path.clone(),
                    query.timeline(),
                    ClearIsRecursive::name(),
                );

                let cache = Arc::clone(
                    self.latest_at_per_cache_key2
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache2::new(key.clone())))),
                );

                let mut cache = cache.write();
                cache.handle_pending_invalidation();
                if let Some(chunk) =
                    cache.latest_at(store, query, &clear_entity_path, ClearIsRecursive::name())
                {
                    // When checking the entity itself, any kind of `Clear` component
                    // (i.e. recursive or not) will do.
                    //
                    // For (recursive) parents, we need to deserialize the data to make sure the
                    // recursive flag is set.
                    #[allow(clippy::collapsible_if)] // readability
                    if clear_entity_path == *entity_path
                        || chunk.component_mono::<ClearIsRecursive>(0)
                            == Some(ClearIsRecursive(true))
                    {
                        let index = chunk
                            .indices(&query.timeline())
                            .and_then(|mut indices| indices.next())
                            .unwrap_or((TimeInt::STATIC, RowId::ZERO)); // TODO: really?
                        if compare_indices(index, max_clear_index) == std::cmp::Ordering::Greater {
                            max_clear_index = index;
                        }
                    }
                }

                let Some(parent_entity_path) = clear_entity_path.parent() else {
                    break;
                };

                clear_entity_path = parent_entity_path;
            }
        }

        let mut components = BTreeMap::new();
        // TODO: except you cannot compound rowids across multiple timelines it just doesnt make sense.
        // really we just need a helper method for when you want the compound result of a
        // QueryResult, that's all there is to it.
        // So you have iter_indexed and compound_indexed for example -- it's fine.
        let mut row_id_max = RowId::ZERO;
        let mut timepoint_max = TimePoint::default();
        let mut compound_index = (TimeInt::STATIC, RowId::ZERO);

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);
            let cache = Arc::clone(
                self.latest_at_per_cache_key2
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache2::new(key.clone())))),
            );

            let mut cache = cache.write();
            cache.handle_pending_invalidation();
            if let Some(chunk) = cache.latest_at(store, query, entity_path, component_name) {
                // 1. A `Clear` component doesn't shadow its own self.
                // 2. If a `Clear` component was found with an index greater than or equal to the
                //    component data, then we know for sure that it should shadow it.
                // TODO
                // if component_name == ClearIsRecursive::name()
                //     || compare_indices(*cached.index(), max_clear_index)
                //         == std::cmp::Ordering::Greater
                // {
                //     results.add(component_name, cached);
                // }

                if let Some(index) = chunk
                    .indices(&query.timeline())
                    .and_then(|mut indices| indices.next())
                {
                    if index > compound_index {
                        compound_index = index;
                    }

                    components.insert(component_name, vec![chunk]);
                }

                // // TODO: this really cannot fail.
                // if let Some(list_array) = chunk.list_array(&component_name) {
                //     // NOTE: Since this is a compound API that actually emits multiple queries, the index of the
                //     // final result is the most recent index among all of its components, as defined by time
                //     // and row-id order.
                //
                //     // TODO: this just cannot fail because reasons
                //     if let Some(index) = chunk
                //         .indices(&query.timeline())
                //         .and_then(|mut indices| indices.next())
                //     {
                //         if index > compound_index {
                //             compound_index = index;
                //         }
                //
                //         components.insert(component_name, list_array.clone());
                //     }
                // }
            }
        }

        // TODO: at this point there is very little reason to not make this a single chunk,
        // possibly?
        // I want to do ranges first and see this looks.

        #[cfg(TODO)]
        {
            Chunk::from_native_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                Some(true),
                &[compound_index.1],
                [(
                    query.timeline(),
                    ChunkTimeline::new(
                        Some(true),
                        query.timeline(),
                        PrimitiveArray::from_vec(vec![compound_index.0.as_i64()]),
                    ),
                )]
                .into_iter()
                .collect(),
                components,
            )
            // TODO: literally cannot fail though, but maybe we return empty otherwise?
            .unwrap()
        }

        QueryResults {
            entity_path: entity_path.clone(),
            kind: QueryResultsKind::LatestAt {
                query: query.clone(),
                compound_index,
            },
            // compound_index,
            components,
            // components: components
            //     .into_iter()
            //     .map(|(component_name, chunks)| {
            //         (
            //             component_name,
            //             chunks
            //                 .into_iter()
            //                 .map(|chunk| {
            //                     Arc::new(
            //                         Chunk::from_native_row_ids(
            //                             chunk.id(),
            //                             chunk.entity_path().clone(),
            //                             Some(true),
            //                             &[compound_index.1],
            //                             // TODO: that one needs to be overriden
            //                             chunk.timelines().clone(),
            //                             chunk.components().clone(),
            //                         )
            //                         .unwrap(),
            //                     )
            //                 })
            //                 .collect(),
            //         )
            //     })
            //     .collect(),
        }
    }
}

// ---

/// Caches the results of `LatestAt` queries for a given [`CacheKey`].
pub struct LatestAtCache2 {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    // TODO: specify that all these chunks are densified etc
    //
    /// Organized by _query_ time.
    ///
    /// If the data you're looking for isn't in here, try partially running the query and check
    /// if there is any data available for the resulting _data_ time in [`Self::per_data_time`].
    //
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_query_time: BTreeMap<TimeInt, Arc<Chunk>>,

    /// Organized by _data_ time.
    ///
    /// Due to how our latest-at semantics work, any number of queries at time `T+n` where `n >= 0`
    /// can result in a data time of `T`.
    //
    // NOTE: `Arc` so we can share buckets across query time & data time.
    pub per_data_time: BTreeMap<TimeInt, Arc<Chunk>>,

    /// These timestamps have been invalidated asynchronously.
    ///
    /// The next time this cache gets queried, it must remove any invalidated entries accordingly.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<TimeInt>,
}

impl LatestAtCache2 {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            per_query_time: Default::default(),
            per_data_time: Default::default(),
            pending_invalidations: Default::default(),
        }
    }
}

impl std::fmt::Debug for LatestAtCache2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            per_query_time,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let mut strings = Vec::new();

        struct StatsPerBucket {
            query_times: BTreeSet<TimeInt>,
            data_time: TimeInt,
            total_size_bytes: u64,
        }

        let mut buckets: IndexMap<_, _> = per_data_time
            .iter()
            .map(|(&data_time, bucket)| {
                (
                    Arc::as_ptr(bucket),
                    StatsPerBucket {
                        query_times: Default::default(),
                        data_time,
                        total_size_bytes: bucket.total_size_bytes(),
                    },
                )
            })
            .collect();

        for (&query_time, bucket) in per_query_time {
            if let Some(bucket) = buckets.get_mut(&Arc::as_ptr(bucket)) {
                bucket.query_times.insert(query_time);
            }
        }

        for bucket in buckets.values() {
            strings.push(format!(
                "query_times=[{}] -> data_time={:?} ({})",
                bucket
                    .query_times
                    .iter()
                    .map(|t| cache_key.timeline.typ().format_utc(*t))
                    .collect_vec()
                    .join(", "),
                bucket.data_time.as_i64(),
                re_format::format_bytes(bucket.total_size_bytes as _),
            ));
        }

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for LatestAtCache2 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations,
        } = self;

        let per_query_time = per_query_time
            .keys()
            .map(|k| k.total_size_bytes())
            .sum::<u64>();
        // NOTE: per query time buckets are just pointers, don't count them.

        let per_data_time_keys = per_data_time
            .keys()
            .map(|k| k.total_size_bytes())
            .sum::<u64>();
        let per_data_time_values = per_data_time
            .values()
            // NOTE: make sure to dereference the Arc, else this will account for zero (assumed amortized!)
            .map(|arc| (**arc).total_size_bytes())
            .sum::<u64>();

        let per_data_time = per_data_time_keys + per_data_time_values;
        let pending_invalidations = pending_invalidations.total_size_bytes();

        per_query_time + per_data_time + pending_invalidations
    }
}

/// Implements the complete end-to-end latest-at logic:
/// * Find all applicable `Chunk`s
/// * Apply a latest-at filter to all of them
/// * Keep the one row with the most recent `RowId`
//
// TODO: more happening now
// TODO: that return value really doesnt make that much sense but eh...
pub fn latest_at2(
    store: &ChunkStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    component_name: ComponentName,
) -> Option<(TimeInt, RowId, Chunk)> {
    store
        .latest_at_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        .map(|chunk| chunk.latest_at(query, component_name))
        // NOTE: At this point, the chunk is either empty or has a single row -- the `RowId` we use
        // is irrelevant.
        .filter_map(|chunk| {
            chunk
                .indices(&query.timeline())
                .and_then(|mut indices| indices.next())
                .map(|index| (index, chunk))
        })
        .max_by_key(|(index, _)| *index)
        .and_then(|(_, chunk)| {
            chunk
                .iter_rows(&query.timeline(), &component_name)
                .next()
                .map(|(data_time, row_id, _)| {
                    // TODO: the whole reason the cache exists: pre-filtered, pre-densified, pre-sliced,
                    // pre-sorted...
                    // It's gonna be the same reason we have a range cache, really.
                    //
                    // Update: and now the cache also exists to amortize backwards linear search
                    // for overlaps.
                    //
                    // That and also the fact that it might have taken a linear search to even get
                    // here.
                    (data_time, row_id, chunk.component_sliced(component_name))
                })
        })
}

impl LatestAtCache2 {
    /// Queries cached latest-at data for a single component.
    pub fn latest_at(
        &mut self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        // TODO: no effing clue why we're passing these parameters, how could it be anything else
        // than what is in the cache key???!
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Option<Arc<Chunk>> {
        re_tracing::profile_scope!("latest_at", format!("{query:?}"));

        let Self {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations: _,
        } = self;

        let query_time_bucket_at_query_time = match per_query_time.entry(query.at()) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                // Fastest path: we have an entry for this exact query time, no need to look any
                // further.
                return Some(Arc::clone(entry.get()));
            }
            std::collections::btree_map::Entry::Vacant(entry) => entry,
        };

        if let Some((data_time, row_id, chunk)) =
            latest_at2(store, query, entity_path, component_name)
        {
            let result_data_time = data_time;
            let result_row_id = row_id;
            let result_chunk = chunk;

            // Fast path: we've run the query and realized that we already have the data for the resulting
            // _data_ time, so let's use that to avoid join & deserialization costs.
            if let Some(data_time_bucket_at_data_time) = per_data_time.get(&result_data_time) {
                query_time_bucket_at_query_time.insert(Arc::clone(data_time_bucket_at_data_time));

                // We now know for a fact that a query at that data time would yield the same
                // results: copy the bucket accordingly so that the next cache hit for that query
                // time ends up taking the fastest path.
                let query_time_bucket_at_data_time = per_query_time.entry(result_data_time);
                query_time_bucket_at_data_time
                    .and_modify(|v| *v = Arc::clone(data_time_bucket_at_data_time))
                    .or_insert(Arc::clone(data_time_bucket_at_data_time));

                return Some(Arc::clone(data_time_bucket_at_data_time));
            }

            let bucket = Arc::new(result_chunk);

            // Slowest path: this is a complete cache miss.
            {
                let query_time_bucket_at_query_time =
                    query_time_bucket_at_query_time.insert(Arc::clone(&bucket));

                let data_time_bucket_at_data_time = per_data_time.entry(result_data_time);
                data_time_bucket_at_data_time
                    .and_modify(|v| *v = Arc::clone(query_time_bucket_at_query_time))
                    .or_insert(Arc::clone(query_time_bucket_at_query_time));
            }

            Some(bucket)
        } else {
            None
        }
    }

    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            per_query_time,
            per_data_time,
            pending_invalidations,
        } = self;

        // First, remove any data indexed by a _query time_ that's more recent than the oldest
        // _data time_ that's been invalidated.
        //
        // Note that this data time might very well be `TimeInt::STATIC`, in which case the entire
        // query-time-based index will be dropped.
        if let Some(&oldest_data_time) = pending_invalidations.first() {
            per_query_time.retain(|&query_time, _| query_time < oldest_data_time);
        }

        // Second, remove any data indexed by _data time_, if it's been invalidated.
        let mut dropped_data_times = Vec::new();
        per_data_time.retain(|data_time, _| {
            if pending_invalidations.contains(data_time) {
                dropped_data_times.push(*data_time);
                false
            } else {
                true
            }
        });

        // TODO(#5974): Because of non-deterministic ordering and parallelism and all things of that
        // nature, it can happen that we try to handle pending invalidations before we even cached
        // the associated data.
        //
        // If that happens, the data will be cached after we've invalidated *nothing*, and will stay
        // there indefinitely since the cache doesn't have a dedicated GC yet.
        //
        // TL;DR: make sure to keep track of pending invalidations indefinitely as long as we
        // haven't had the opportunity to actually invalidate the associated data.
        for data_time in dropped_data_times {
            pending_invalidations.remove(&data_time);
        }
    }
}

// ---

// TODO: and now for the big question -- how do we merge latestat and range in a nice way? maybe we
// just stick an enum in there so we can keep track of how to do the range zip, and that's it.

// TODO

#[derive(Debug, Clone)]
pub enum QueryResultsKind {
    LatestAt {
        query: LatestAtQuery,

        /// The compound index of this query result.
        ///
        /// A latest-at query is a compound operation that gathers data from many different rows.
        /// The index of that compound result corresponds to the index of most the recent row in all the
        /// sub-results, as defined by time and row-id order.
        //
        // we could fill that even for a range result -- screw it
        //
        // TODO: should we remove this?
        compound_index: (TimeInt, RowId),
    },
    Range {
        query: RangeQuery,
    },
}

// TODO: we be going for it: one result type for everything -- screw it.
//
/// Results for a latest-at query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`LatestAtResults::get`], [`LatestAtResults::get_required`] and
/// [`LatestAtResults::get_or_empty`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct QueryResults {
    pub entity_path: EntityPath,

    pub kind: QueryResultsKind,

    /// Results for each individual component.
    pub components: BTreeMap<ComponentName, Vec<Arc<Chunk>>>,
}

impl QueryResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(&self, component_name: impl Into<ComponentName>) -> Option<&Arc<Chunk>> {
        self.components.get(&component_name.into())?.first()
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> crate::Result<&Arc<Chunk>> {
        let component_name = component_name.into();
        self.get(component_name)
            .ok_or_else(|| crate::QueryError::PrimaryNotFound(component_name))
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_or_empty(&self, component_name: impl Into<ComponentName>) -> Arc<Chunk> {
        self.get(component_name)
            .cloned()
            .unwrap_or_else(|| Arc::new(Chunk::empty(ChunkId::ZERO, self.entity_path.clone())))
    }

    /// Utility for retrieving a single instance of a component.
    #[inline]
    pub fn get_instance<C: re_types_core::Component>(&self, index: usize) -> Option<C> {
        self.get(C::name())
            .and_then(|chunk| chunk.component_instance(0, index))
    }

    #[inline]
    pub fn get_vec<C: re_types_core::Component>(&self) -> Option<Vec<C>> {
        self.get(C::name())
            .and_then(|chunk| chunk.component_batch(0))
    }

    #[inline]
    pub fn iter_indices<'a, C: re_types_core::Component>(
        &'a self,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + 'a {
        let Some(chunks) = self.components.get(&C::name()) else {
            return Either::Left(std::iter::empty());
        };

        match &self.kind {
            QueryResultsKind::LatestAt {
                query: _,
                compound_index,
            } => Either::Right(Either::Left(std::iter::once(*compound_index))),
            QueryResultsKind::Range { query } => {
                // TODO: we can and should assume that the chunks are already ordered as much as possible (meaning: overlaps!)
                Either::Right(Either::Right(
                    chunks
                        .iter()
                        .flat_map(|chunk| chunk.indices(&query.timeline()).unwrap()),
                ))
            }
        }
    }

    #[inline]
    pub fn iter_batches<'a, C: re_types_core::Component>(
        &'a self,
    ) -> impl Iterator<Item = Vec<C>> + 'a {
        let Some(chunks) = self.components.get(&C::name()) else {
            return Either::Left(std::iter::empty());
        };

        // TODO: we can and should assume that the chunks are already ordered as much as possible (meaning: overlaps!)
        Either::Right(chunks.iter().flat_map(|chunk| chunk.iter_batches()))
    }

    // TODO: that's the dumb way of accessing data
    // TODO
    #[inline]
    pub fn iter_indexed<'a, C: re_types_core::Component>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<C>)> + 'a {
        izip!(self.iter_indices::<C>(), self.iter_batches::<C>())

        // let Some(chunks) = self.components.get(&C::name()) else {
        //     return Either::Left(std::iter::empty());
        // };
        //
        //
        // let timeline = match &self.kind {
        //     QueryResultsKind::LatestAt {
        //         query,
        //         compound_index: _,
        //     } => query.timeline(),
        //     QueryResultsKind::Range { query } => query.timeline(),
        // };
        //
        // // TODO: we can and should assume that the chunks are already ordered as much as possible (meaning: overlaps!)
        // Either::Right(
        //     chunks
        //         .iter()
        //         .flat_map(move |chunk| chunk.iter_indexed(&timeline)),
        // )
    }

    // TODO: we could generate some range_zip helpers here eh?
}
