use std::{collections::BTreeSet, sync::Arc};

use ahash::HashMap;
use arrow2::array::Array;
use itertools::Itertools;
use parking_lot::RwLock;

use re_chunk::{Chunk, ChunkId, ChunkShared, RowId};
use re_chunk_store::{ChunkStore, LatestAtQuery, RangeQuery, TimeInt};
use re_log_types::{EntityPath, ResolvedTimeRange};
use re_types_core::{ComponentName, SizeBytes};

use crate::{CacheKey, Caches, RangeResults};

// ---

impl Caches {
    /// Queries for the given `component_names` using range semantics.
    ///
    /// See [`RangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range(
        &self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> RangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = RangeResults::new(query.clone());

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = Arc::clone(
                self.range_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(RangeCache::new(key.clone())))),
            );

            let mut cache = cache.write();

            cache.handle_pending_invalidation();

            let cached = cache.range(store, query, entity_path, component_name);
            results.add(component_name, cached);
        }

        results
    }
}

// ---

/// Caches the results of `Range` queries for a given [`CacheKey`].
pub struct RangeCache {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    // TODO
    // TODO: reminder: these are sorted and densified
    pub chunks: HashMap<ChunkId, Chunk>,

    /// Everything greater than or equal to this timestamp has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching this criteria.
    /// `None` indicates that there's no pending invalidation.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<ChunkId>,
}

impl RangeCache {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            chunks: HashMap::default(),
            pending_invalidations: BTreeSet::default(),
        }
    }
}

impl std::fmt::Debug for RangeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            chunks,
            pending_invalidations: _,
        } = self;

        let mut strings: Vec<String> = Vec::new();

        let mut data_time_min = TimeInt::MAX;
        let mut data_time_max = TimeInt::MIN;

        #[cfg(TODO)]
        {
            let per_data_time = per_data_time.read();

            let per_data_time_indices = &per_data_time.indices;
            if let Some(time_front) = per_data_time_indices.front().map(|(t, _)| *t) {
                data_time_min = TimeInt::min(data_time_min, time_front);
            }
            if let Some(time_back) = per_data_time_indices.back().map(|(t, _)| *t) {
                data_time_max = TimeInt::max(data_time_max, time_back);
            }
        }

        #[cfg(TODO)]
        strings.push(format!(
            "{} ({})",
            cache_key
                .timeline
                .typ()
                .format_range_utc(ResolvedTimeRange::new(data_time_min, data_time_max)),
            re_format::format_bytes(per_data_time.total_size_bytes() as _),
        ));

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl SizeBytes for RangeCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key,
            chunks,
            pending_invalidations,
        } = self;

        cache_key.heap_size_bytes()
            + chunks.heap_size_bytes()
            + pending_invalidations.heap_size_bytes()
    }
}

impl RangeCache {
    /// Queries cached range data for a single component.
    pub fn range(
        &mut self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Vec<Chunk> {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        // TODO: how the hell would we know whether something is missing, though?
        // I guess we could always fetch all the relevant chunks from the real store, which is
        // quite cheap anyhow, and then check whether we already have them sorted... which the does
        // on its own anyhow, right?
        //
        // or maybe we literally just keep a map of chunks to chunks, but pre-processed?

        // TODO: this is literally the same cache as the LatestAt one... this is a ChunkCache,
        // nothing else.

        let raw_chunks = store.range_relevant_chunks(query, entity_path, component_name);
        for raw_chunk in &raw_chunks {
            // TODO: only sorting, no actual ranging
            self.chunks
                .entry(raw_chunk.id())
                .or_insert_with(|| raw_chunk.sorted_by_timeline_if_unsorted(&query.timeline()));
        }

        raw_chunks
            .into_iter()
            .filter_map(|raw_chunk| self.chunks.get(&raw_chunk.id()))
            .map(|cached_sorted_chunk| {
                debug_assert!(cached_sorted_chunk.is_timeline_sorted(&query.timeline()));
                cached_sorted_chunk.range(query, component_name)
            })
            .filter(|chunk| !chunk.is_empty())
            .collect()
    }

    #[inline]
    pub fn handle_pending_invalidation(&mut self) {
        re_tracing::profile_function!();

        let Self {
            cache_key: _,
            chunks,
            pending_invalidations,
        } = self;

        chunks.retain(|chunk_id, _chunk| !pending_invalidations.contains(&chunk_id));
    }
}
