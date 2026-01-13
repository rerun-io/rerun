use std::collections::BTreeSet;
use std::sync::Arc;

use ahash::HashMap;
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier};
use re_chunk_store::{ChunkStore, OnMissingChunk, RangeQuery, TimeInt};
use re_log_types::{AbsoluteTimeRange, EntityPath};

use crate::{QueryCache, QueryCacheKey, QueryError};

// --- Public API ---

impl QueryCache {
    /// Queries for the given components using range semantics.
    ///
    /// See [`RangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range(
        &self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> RangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let store = self.store.read();

        let mut results = RangeResults::new(query.clone());

        // NOTE: This pre-filtering is extremely important: going through all these query layers
        // has non-negligible overhead even if the final result ends up being nothing, and our
        // number of queries for a frame grows linearly with the number of entity paths.
        let components = components.into_iter().filter(|component_identifier| {
            store.entity_has_component_on_timeline(
                query.timeline(),
                entity_path,
                *component_identifier,
            )
        });

        for component in components {
            let key = QueryCacheKey::new(entity_path.clone(), *query.timeline(), component);

            let cache = Arc::clone(
                self.range_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(RangeCache::new(key)))),
            );

            let mut cache = cache.write();

            cache.handle_pending_invalidation();

            let (cached, missing) = cache.range(&store, query, entity_path, component);
            results.missing.extend(missing);
            if !cached.is_empty() {
                results.add(component, cached);
            }
        }

        results
    }
}

// --- Results ---

/// Results for a range query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`RangeResults::get`] or [`RangeResults::get_required`] in order to access the results for
/// each individual component.
///
/// Since the introduction of virtual/offloaded chunks, it is possible for a query to detect that
/// it is missing some data in order to compute accurate results.
/// This lack of data is communicated using a non-empty [`RangeResults::missing`] field.
#[derive(Debug, PartialEq)]
pub struct RangeResults {
    /// The query that yielded these results.
    pub query: RangeQuery,

    /// The relevant *virtual* chunks that were found for this query.
    ///
    /// Until these chunks have been fetched and inserted into the appropriate [`ChunkStore`], the
    /// results of this query cannot accurately be computed.
    //
    // TODO(cmc): Once lineage tracking is in place, make sure that this only reports missing
    // chunks using their root-level IDs, so downstream consumers don't have to redundantly build
    // their own tracking. And document it so.
    pub missing: Vec<ChunkId>,

    /// Results for each individual component.
    pub components: IntMap<ComponentIdentifier, Vec<Chunk>>,
}

impl RangeResults {
    /// Returns true if these are partial results.
    ///
    /// Partial results happen when some of the chunks required to accurately compute the query are
    /// currently missing/offloaded.
    /// It is then the responsibility of the caller to look into the [missing chunk IDs], fetch
    /// them, load them, and then try the query again.
    ///
    /// [missing chunk IDs]: `Self::missing`
    pub fn is_partial(&self) -> bool {
        !self.missing.is_empty()
    }

    /// Returns true if the results are *completely* empty.
    ///
    /// I.e. neither physical/loaded nor virtual/offloaded chunks could be found.
    pub fn is_empty(&self) -> bool {
        let Self {
            query: _,
            missing,
            components,
        } = self;
        missing.is_empty() && components.values().all(|chunks| chunks.is_empty())
    }

    /// Returns the [`Chunk`]s for the specified component.
    #[inline]
    pub fn get(&self, component: ComponentIdentifier) -> Option<&[Chunk]> {
        self.components
            .get(&component)
            .map(|chunks| chunks.as_slice())
    }

    /// Returns the [`Chunk`]s for the specified component.
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(&self, component: ComponentIdentifier) -> crate::Result<&[Chunk]> {
        self.components.get(&component).map_or_else(
            || Err(QueryError::PrimaryNotFound(component)),
            |chunks| Ok(chunks.as_slice()),
        )
    }
}

impl RangeResults {
    #[inline]
    fn new(query: RangeQuery) -> Self {
        Self {
            query,
            missing: Default::default(),
            components: Default::default(),
        }
    }

    #[inline]
    fn add(&mut self, component: ComponentIdentifier, chunks: Vec<Chunk>) {
        self.components.insert(component, chunks);
    }
}

// --- Cache implementation ---

/// Caches the results of `Range` queries for a given [`QueryCacheKey`].
pub struct RangeCache {
    /// For debugging purposes.
    pub cache_key: QueryCacheKey,

    /// All the [`Chunk`]s currently cached.
    ///
    /// See [`RangeCachedChunk`] for more information.
    pub chunks: HashMap<ChunkId, RangeCachedChunk>,

    /// Every [`ChunkId`] present in this set has been asynchronously invalidated.
    ///
    /// The next time this cache gets queried, it must remove any entry matching any of these IDs.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<ChunkId>,
}

impl RangeCache {
    #[inline]
    pub fn new(cache_key: QueryCacheKey) -> Self {
        Self {
            cache_key,
            chunks: HashMap::default(),
            pending_invalidations: BTreeSet::default(),
        }
    }

    /// Returns the time range covered by this [`RangeCache`].
    ///
    /// This is extremely slow (`O(n)`), don't use this for anything but debugging.
    #[inline]
    pub fn time_range(&self) -> AbsoluteTimeRange {
        self.chunks
            .values()
            .filter_map(|cached| {
                cached
                    .chunk
                    .timelines()
                    .get(&self.cache_key.timeline_name)
                    .map(|time_column| time_column.time_range())
            })
            .fold(AbsoluteTimeRange::EMPTY, |mut acc, time_range| {
                acc.set_min(TimeInt::min(acc.min(), time_range.min()));
                acc.set_max(TimeInt::max(acc.max(), time_range.max()));
                acc
            })
    }
}

impl std::fmt::Debug for RangeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key: _,
            chunks,
            pending_invalidations: _,
        } = self;

        let mut strings: Vec<String> = Vec::new();

        strings.push(format!(
            "{:?} ({})",
            self.time_range(),
            re_format::format_bytes(chunks.total_size_bytes() as _),
        ));

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

pub struct RangeCachedChunk {
    pub chunk: Chunk,

    /// When a `Chunk` gets cached, it is pre-processed according to the current [`QueryCacheKey`],
    /// e.g. it is time-sorted on the appropriate timeline.
    ///
    /// In the happy case, pre-processing a `Chunk` is a no-op, and the cached `Chunk` is just a
    /// reference to the real one sitting in the store.
    /// Otherwise, the cached `Chunk` is a full blown copy of the original one.
    pub resorted: bool,
}

impl SizeBytes for RangeCachedChunk {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { chunk, resorted } = self;

        if *resorted {
            // The chunk had to be post-processed for caching.
            // Its data was duplicated.
            Chunk::heap_size_bytes(chunk)
        } else {
            // This chunk is just a reference to the one in the store.
            // Consider it amortized.
            0
        }
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
    ///
    /// This returns the cached physical chunks that were found for this query, as well as any
    /// virtual chunks that need to be fetched and loaded.
    /// It is then the responsibility of the caller to look into these missing chunk IDs, fetch
    /// them, load them, and then try the query again.
    ///
    /// Returns `(cached_chunks, missing_chunk_ids)`.
    fn range(
        &mut self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> (Vec<Chunk>, Vec<ChunkId>) {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        debug_assert_eq!(query.timeline(), &self.cache_key.timeline_name);

        // First, we forward the query as-is to the store.
        //
        // It's fine to run the query every time -- the index scan itself is not the costly part of a
        // range query.
        //
        // For all relevant chunks that we find, we process them according to the [`QueryCacheKey`], and
        // cache them.

        let results = store.range_relevant_chunks(query, entity_path, component);
        // It is perfectly safe to cache partial range results, since missing data (if any), cannot
        // possibly affect what's already cached, it can only augment it.
        // Therefore, we do not even check for partial results here.
        for raw_chunk in &results.chunks {
            self.chunks
                .entry(raw_chunk.id())
                .or_insert_with(|| RangeCachedChunk {
                    // TODO(#7008): avoid unnecessary sorting on the unhappy path
                    chunk: raw_chunk
                        // Densify the cached chunk according to the cache key's component, which
                        // will speed up future arrow operations on this chunk.
                        .densified(component)
                        // Pre-sort the cached chunk according to the cache key's timeline.
                        .sorted_by_timeline_if_unsorted(&self.cache_key.timeline_name),

                    // TODO(cmc): this isn't good enough: if the chunk was indeed densified, then we
                    // need to account for it in the memory stats, whether it was resorted or not.
                    resorted: !raw_chunk.is_timeline_sorted(&self.cache_key.timeline_name),
                });
        }

        // Second, we simply retrieve from the cache all the relevant `Chunk`s .
        //
        // Since these `Chunk`s have already been pre-processed adequately, running a range filter
        // on them will be quite cheap.

        // It is perfectly fine to return partial range results, as they are always valid on their own,
        // as long as we also advertise that some chunks were missing (which we do).
        // Therefore, we do not even check for partial results here.
        let chunks = results
            .chunks
            .into_iter()
            .filter_map(|raw_chunk| self.chunks.get(&raw_chunk.id()))
            .map(|cached_sorted_chunk| {
                debug_assert!(
                    cached_sorted_chunk
                        .chunk
                        .is_timeline_sorted(query.timeline())
                );

                let chunk = &cached_sorted_chunk.chunk;

                chunk.range(query, component)
            })
            .filter(|chunk| !chunk.is_empty())
            .collect();

        (chunks, results.missing)
    }

    #[inline]
    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            chunks,
            pending_invalidations,
        } = self;

        chunks.retain(|chunk_id, _chunk| !pending_invalidations.contains(chunk_id));

        pending_invalidations.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, ChunkId, RowId};
    use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::external::re_tuid::Tuid;
    use re_log_types::{EntityPath, TimePoint, Timeline};

    use super::*;

    // Make sure queries yield partial results when we expect them to.
    #[test]
    #[expect(clippy::bool_assert_comparison)] // I like it that way, sue me
    fn partial_data_basics() {
        let store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let store = ChunkStoreHandle::new(store);

        let entity_path: EntityPath = "some_entity".into();

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint1 = TimePoint::from_iter([(timeline_frame, 1)]);
        let point1 = MyPoint::new(1.0, 1.0);

        let mut next_chunk_id = next_chunk_id_generator(0x1337);

        // Overlapped chunks!
        let chunk1 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint1.clone(),
            point1,
        );
        let chunk2 = chunk1.clone_as(next_chunk_id(), RowId::new());
        let chunk3 = chunk2.clone_as(next_chunk_id(), RowId::new());

        let cache = QueryCache::new(store.clone());

        let component = MyPoints::descriptor_points().component;
        let query = RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3));

        // We haven't inserted anything yet, so we just expect empty results across the board.
        {
            let results = cache.range(&query, &entity_path, [component]);
            assert!(results.is_empty());
        }

        // Reminder: the store events are irrelevant here, since the range cache still always unconditionally
        // performs the underlying query regardless (only the sorting/slicing is cached).
        store
            .write()
            .insert_chunk(&Arc::new(chunk1.clone()))
            .unwrap();
        store
            .write()
            .insert_chunk(&Arc::new(chunk2.clone()))
            .unwrap();
        store
            .write()
            .insert_chunk(&Arc::new(chunk3.clone()))
            .unwrap();

        // Now we've inserted everything, so we expect complete results across the board.
        {
            let results = cache.range(&query, &entity_path, [component]);
            let expected = {
                let mut results = RangeResults::new(query.clone());
                results.add(
                    component,
                    vec![chunk1.clone(), chunk2.clone(), chunk3.clone()],
                );
                results
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);
        }

        // Reminder: the store events are irrelevant here, since the range cache still always unconditionally
        // performs the underlying query regardless (only the sorting/slicing is cached).
        store.write().remove_chunks_shallow(
            vec![Arc::new(chunk1.clone()), Arc::new(chunk3.clone())],
            None,
        );

        // We've removed the first and last chunks from the store: results should now be partial.
        {
            let results = cache.range(&query, &entity_path, [component]);
            let expected = {
                let mut results = RangeResults::new(query.clone());
                results.add(component, vec![chunk2.clone()]);
                results.missing = vec![chunk1.id(), chunk3.id()];
                results
            };
            assert_eq!(true, results.is_partial());
            assert_eq!(expected, results);
        }

        // Reminder: the store events are irrelevant here, since the range cache still always unconditionally
        // performs the underlying query regardless (only the sorting/slicing is cached).
        store
            .write()
            .remove_chunks_shallow(vec![Arc::new(chunk2.clone())], None);

        // Now we've removed absolutely everything: we should only get partial results.
        {
            let results = cache.range(&query, &entity_path, [component]);
            let expected = {
                let mut results = RangeResults::new(query.clone());
                results.missing = vec![chunk1.id(), chunk2.id(), chunk3.id()];
                results
            };
            assert_eq!(true, results.is_partial());
            assert_eq!(expected, results);
        }

        // Reminder: the store events are irrelevant here, since the range cache still always unconditionally
        // performs the underlying query regardless (only the sorting/slicing is cached).
        store
            .write()
            .insert_chunk(&Arc::new(chunk1.clone()))
            .unwrap();
        store
            .write()
            .insert_chunk(&Arc::new(chunk2.clone()))
            .unwrap();
        store
            .write()
            .insert_chunk(&Arc::new(chunk3.clone()))
            .unwrap();

        // We've inserted everything back: all results should be complete once again.
        {
            let results = cache.range(&query, &entity_path, [component]);
            let expected = {
                let mut results = RangeResults::new(query.clone());
                results.add(
                    component,
                    vec![chunk1.clone(), chunk2.clone(), chunk3.clone()],
                );
                results
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);
        }
    }

    fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
        let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
        move || {
            chunk_id = chunk_id.next();
            chunk_id
        }
    }

    fn create_chunk_with_point(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        timepoint: TimePoint,
        point: MyPoint,
    ) -> Chunk {
        Chunk::builder_with_id(chunk_id, entity_path)
            .with_component_batch(
                RowId::new(),
                timepoint,
                (MyPoints::descriptor_points(), &[point]),
            )
            .build()
            .unwrap()
    }
}
