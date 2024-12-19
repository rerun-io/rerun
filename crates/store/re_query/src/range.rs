use std::{borrow::Cow, collections::BTreeSet, sync::Arc};

use ahash::HashMap;
use nohash_hasher::IntMap;
use parking_lot::RwLock;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ChunkId};
use re_chunk_store::{ChunkStore, RangeQuery, TimeInt};
use re_log_types::{EntityPath, ResolvedTimeRange};
use re_types_core::{ComponentDescriptor, ComponentName, DeserializationError};

use crate::{QueryCache, QueryCacheKey};

// --- Public API ---

impl QueryCache {
    /// Queries for the given `component_names` using range semantics.
    ///
    /// See [`RangeResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn range<'a>(
        &self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_descrs: impl IntoIterator<Item = impl Into<Cow<'a, ComponentDescriptor>>>,
    ) -> RangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let store = self.store.read();

        let mut results = RangeResults::new(query.clone());

        // NOTE: This pre-filtering is extremely important: going through all these query layers
        // has non-negligible overhead even if the final result ends up being nothing, and our
        // number of queries for a frame grows linearly with the number of entity paths.
        let component_names = component_descrs.into_iter().filter_map(|component_descr| {
            let component_descr = component_descr.into();
            store
                .entity_has_component_on_timeline(
                    &query.timeline(),
                    entity_path,
                    &component_descr.component_name,
                )
                .then_some(component_descr.component_name)
        });

        for component_name in component_names {
            let key = QueryCacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = Arc::clone(
                self.range_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(RangeCache::new(key.clone())))),
            );

            let mut cache = cache.write();

            cache.handle_pending_invalidation();

            let cached = cache.range(&store, query, entity_path, component_name);
            if !cached.is_empty() {
                results.add(component_name, cached);
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
#[derive(Debug)]
pub struct RangeResults {
    /// The query that yielded these results.
    pub query: RangeQuery,

    /// Results for each individual component.
    pub components: IntMap<ComponentName, Vec<Chunk>>,
}

impl RangeResults {
    #[inline]
    pub fn new(query: RangeQuery) -> Self {
        Self {
            query,
            components: Default::default(),
        }
    }

    #[inline]
    pub fn contains(&self, component_name: &ComponentName) -> bool {
        self.components.contains_key(component_name)
    }

    /// Returns the [`Chunk`]s for the specified `component_name`.
    #[inline]
    pub fn get(&self, component_name: &ComponentName) -> Option<&[Chunk]> {
        self.components
            .get(component_name)
            .map(|chunks| chunks.as_slice())
    }

    /// Returns the [`Chunk`]s for the specified `component_name`.
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(&self, component_name: &ComponentName) -> crate::Result<&[Chunk]> {
        if let Some(chunks) = self.components.get(component_name) {
            Ok(chunks)
        } else {
            Err(DeserializationError::MissingComponent {
                component: *component_name,
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }
}

impl RangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, chunks: Vec<Chunk>) {
        self.components.insert(component_name, chunks);
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
    pub fn time_range(&self) -> ResolvedTimeRange {
        self.chunks
            .values()
            .filter_map(|cached| {
                cached
                    .chunk
                    .timelines()
                    .get(&self.cache_key.timeline)
                    .map(|time_column| time_column.time_range())
            })
            .fold(ResolvedTimeRange::EMPTY, |mut acc, time_range| {
                acc.set_min(TimeInt::min(acc.min(), time_range.min()));
                acc.set_max(TimeInt::max(acc.max(), time_range.max()));
                acc
            })
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

        strings.push(format!(
            "{} ({})",
            cache_key.timeline.typ().format_range_utc(self.time_range()),
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
    pub fn range(
        &mut self,
        store: &ChunkStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Vec<Chunk> {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        debug_assert_eq!(query.timeline(), self.cache_key.timeline);

        // First, we forward the query as-is to the store.
        //
        // It's fine to run the query every time -- the index scan itself is not the costly part of a
        // range query.
        //
        // For all relevant chunks that we find, we process them according to the [`QueryCacheKey`], and
        // cache them.

        let raw_chunks = store.range_relevant_chunks(query, entity_path, component_name);
        for raw_chunk in &raw_chunks {
            self.chunks
                .entry(raw_chunk.id())
                .or_insert_with(|| RangeCachedChunk {
                    // TODO(#7008): avoid unnecessary sorting on the unhappy path
                    chunk: raw_chunk
                        // Densify the cached chunk according to the cache key's component, which
                        // will speed up future arrow operations on this chunk.
                        .densified(component_name)
                        // Pre-sort the cached chunk according to the cache key's timeline.
                        .sorted_by_timeline_if_unsorted(&self.cache_key.timeline),
                    resorted: !raw_chunk.is_timeline_sorted(&self.cache_key.timeline),
                });
        }

        // Second, we simply retrieve from the cache all the relevant `Chunk`s .
        //
        // Since these `Chunk`s have already been pre-processed adequately, running a range filter
        // on them will be quite cheap.

        raw_chunks
            .into_iter()
            .filter_map(|raw_chunk| self.chunks.get(&raw_chunk.id()))
            .map(|cached_sorted_chunk| {
                debug_assert!(cached_sorted_chunk
                    .chunk
                    .is_timeline_sorted(&query.timeline()));

                let chunk = &cached_sorted_chunk.chunk;

                chunk.range(query, component_name)
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

        chunks.retain(|chunk_id, _chunk| !pending_invalidations.contains(chunk_id));

        pending_invalidations.clear();
    }
}
