use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arrow2::{array::Array, Either};
use itertools::Itertools;
use parking_lot::RwLock;

use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::{ChunkStore, LatestAtQuery, RangeQuery, TimeInt};
use re_log_types::{EntityPath, ResolvedTimeRange};
use re_types_core::{ComponentName, SizeBytes};

use crate::{
    CacheKey, Caches, QueryResults, QueryResultsKind, RangeComponentResults,
    RangeComponentResultsInner, RangeResults,
};

// ---

impl Caches {
    /// Queries for the given `component_names` using range semantics.
    ///
    /// See [`RangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range2(
        &self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> QueryResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut components = BTreeMap::new();

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = Arc::clone(
                self.range_per_cache_key2
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(RangeCache2::new(key.clone())))),
            );

            let mut cache = cache.write();

            // TODO: we can avoid having to do any of this if we just query everything always, and
            // then filter out the chunk ids we already know about.
            // We can afford doing so now that we work a whole chunk at a time rather than a row at
            // a time.
            #[cfg(TODO)]
            //
            // TODO(#4810): Get rid of this once we have proper bucketing in place.
            //
            // Detects the case where the user loads a piece of data at the end of the time range, then a piece
            // at the beginning of the range, and finally a piece right in the middle.
            //
            // DATA = ###################################################
            //          |      |     |       |            \_____/
            //          \______/     |       |            query #1
            //          query #2     \_______/
            //                       query #3
            //
            // and coarsly invalidates the whole cache in that case, to avoid the kind of bugs
            // showcased in <https://github.com/rerun-io/rerun/issues/5686>.
            {
                let time_range = cache.per_data_time.read_recursive().pending_time_range();
                if let Some(time_range) = time_range {
                    {
                        let hole_start = time_range.max();
                        let hole_end =
                            TimeInt::new_temporal(query.range().min().as_i64().saturating_sub(1));
                        if hole_start < hole_end {
                            let query = &LatestAtQuery::new(query.timeline(), hole_end);
                            if let Some((data_time, _, _)) =
                                crate::latest_at(store, query, entity_path, component_name)
                            {
                                if data_time > hole_start {
                                    re_log::trace!(%entity_path, %component_name, "coarsely invalidated because of bridged queries");
                                    cache.pending_invalidation = Some(TimeInt::MIN);
                                }
                            }
                        }
                    }

                    {
                        let hole_start = query.range().max();
                        let hole_end =
                            TimeInt::new_temporal(time_range.min().as_i64().saturating_sub(1));
                        if hole_start < hole_end {
                            let query = &LatestAtQuery::new(query.timeline(), hole_end);
                            if let Some((data_time, _, _)) =
                                crate::latest_at(store, query, entity_path, component_name)
                            {
                                if data_time > hole_start {
                                    re_log::trace!(%entity_path, %component_name, "coarsely invalidated because of bridged queries");
                                    cache.pending_invalidation = Some(TimeInt::MIN);
                                }
                            }
                        }
                    }
                }
            }

            cache.handle_pending_invalidation();

            let chunks = cache
                .range(store, query, entity_path, component_name)
                .collect_vec();

            components.insert(component_name, chunks);
        }

        // results

        QueryResults {
            entity_path: entity_path.clone(),
            kind: QueryResultsKind::Range {
                query: query.clone(),
            },
            components,
        }
    }
}

// ---

/// Caches the results of `Range` queries for a given [`CacheKey`].
pub struct RangeCache2 {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    // TODO: so we can know what we already have
    pub chunk_ids: BTreeSet<ChunkId>,

    // TODO: specify that all these chunks are densified etc
    //
    /// All temporal data, organized by _data_ time.
    ///
    /// Query time is irrelevant for range queries.
    pub per_start_data_time: BTreeMap<TimeInt, Vec<Arc<Chunk>>>,

    /// Everything greater than or equal to this timestamp has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching this criteria.
    /// `None` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidation: Option<TimeInt>,
}

impl RangeCache2 {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            chunk_ids: Default::default(),
            per_start_data_time: Default::default(),
            pending_invalidation: None,
        }
    }
}

// TODO
impl std::fmt::Debug for RangeCache2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            chunk_ids,
            per_start_data_time,
            pending_invalidation: _,
        } = self;

        let mut strings = Vec::new();

        let mut data_time_min = TimeInt::MAX;
        let mut data_time_max = TimeInt::MIN;

        let data_time_min = per_start_data_time
            .first_key_value()
            .map(|(&data_time, _)| data_time)
            .unwrap_or(TimeInt::MAX);
        let data_time_max = per_start_data_time
            .last_key_value()
            .map(|(&data_time, _)| data_time)
            .unwrap_or(TimeInt::MIN);

        strings.push(format!(
            "{} ({})",
            cache_key
                .timeline
                .typ()
                .format_range_utc(ResolvedTimeRange::new(data_time_min, data_time_max)),
            re_format::format_bytes(per_start_data_time.total_size_bytes() as _),
        ));

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for RangeCache2 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key,
            chunk_ids,
            per_start_data_time: per_data_time,
            pending_invalidation,
        } = self;

        cache_key.heap_size_bytes()
            + chunk_ids.heap_size_bytes()
            + per_data_time.heap_size_bytes()
            + pending_invalidation.heap_size_bytes()
    }
}

/// Implements the complete end-to-end range logic:
/// * Find all applicable `Chunk`s
/// * Apply a range filter to all of them
/// * Concatenate all the results (will sort them later)
pub fn range2<'a>(
    store: &'a ChunkStore,
    query: &'a RangeQuery,
    entity_path: &EntityPath,
    component_name: ComponentName,
    already_cached: &'a BTreeSet<ChunkId>,
) -> impl Iterator<Item = Chunk> + 'a {
    store
        .range_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        // TODO: filtering needs to happen before we even do the look up really.
        .filter(|chunk| !already_cached.contains(&chunk.id()))
        .map(move |chunk| chunk.range(query, component_name))
        .filter(|chunk| !chunk.is_empty())
        // TODO: how could that _not_ be the case though
        .filter(|chunk| chunk.is_static() || chunk.timelines().contains_key(&query.timeline()))
        .map(move |chunk| {
            chunk
                .component_sliced(component_name)
                .densified(component_name)
                .sorted_by_timeline_if_unsorted(&query.timeline())
        })
}

impl RangeCache2 {
    /// Queries cached range data for a single component.
    pub fn range(
        &mut self,
        store: &ChunkStore,
        query: &RangeQuery,
        // TODO: no effing clue why we're passing these parameters, how could it be anything else
        // than what is in the cache key???!
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> impl Iterator<Item = Arc<Chunk>> + '_ {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        let Self {
            cache_key: _,
            chunk_ids,
            per_start_data_time,
            pending_invalidation: _,
        } = self;

        // TODO: static thingies should push away everything else... but that should already be
        // taken care of by the invalidation subsystem

        let chunks = range2(store, query, entity_path, component_name, chunk_ids)
            .map(|chunk| {
                let data_time_min = chunk
                    .timelines()
                    .get(&query.timeline())
                    .and_then(|time_chunk| time_chunk.times().next())
                    .unwrap_or(TimeInt::STATIC); // TODO: explain
                (data_time_min, chunk)
            })
            .collect_vec();

        chunk_ids.extend(chunks.iter().map(|(_, chunk)| chunk.id()));

        for (data_time_min, chunk) in chunks {
            per_start_data_time
                .entry(data_time_min)
                .or_default()
                .push(Arc::new(chunk));
        }

        if let Some(chunks) = per_start_data_time.get(&TimeInt::STATIC) {
            return Either::Left(chunks.clone().into_iter());
        }

        let start_time = per_start_data_time
            .range(..=query.range.min())
            .next_back()
            .map_or(TimeInt::MIN, |(&time, _)| time);

        let end_time = per_start_data_time
            .range(..=query.range.max())
            .next_back()
            .map_or(start_time, |(&time, _)| time);

        // NOTE: Just being extra cautious because, even though this shouldnt possibly ever happen,
        // indexing a std map with a backwards range is an instant crash.
        let end_time = TimeInt::max(start_time, end_time);

        Either::Right(
            per_start_data_time
                .range(start_time..=end_time)
                .map(|(_time, chunks)| chunks)
                .flatten()
                .map(Arc::clone),
        )
    }

    pub fn handle_pending_invalidation(&mut self) {
        re_tracing::profile_function!();

        let Self {
            cache_key: _,
            chunk_ids,
            per_start_data_time,
            pending_invalidation,
        } = self;

        let Some(pending_invalidation) = pending_invalidation.take() else {
            return;
        };

        // TODO: do the real stuff
        per_start_data_time.clear();
        chunk_ids.clear();

        // TODO

        // TODO: think about this
        //
        // Invalidating data is tricky. Our results object may have been cloned and shared already.
        // We can't just invalidate the data in-place without guaranteeing the post-invalidation query
        // will return the same results as the pending pre-invalidation queries.
        //
        // let mut new_inner = (*per_data_time.read()).clone();
        // new_inner.truncate_at_time(pending_invalidation);
        // per_data_time.inner = Arc::new(RwLock::new(new_inner));
    }
}
