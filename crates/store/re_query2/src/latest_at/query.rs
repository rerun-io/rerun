use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use arrow2::array::Array as ArrowArray;
use indexmap::IndexMap;
use itertools::Itertools;
use parking_lot::RwLock;

use re_chunk::{Chunk, ChunkId, ChunkSharedMono, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, LatestAtQuery, TimeInt};
use re_log_types::EntityPath;
use re_types_core::{components::ClearIsRecursive, ComponentName, Loggable as _, SizeBytes};

use crate::{CacheKey, Caches, LatestAtResults};

// ---

// TODO: latest-at queries have to be fully cached, there's just no way around it, it's too hot.

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

// TODO: wait... could each cache just be store????

impl Caches {
    /// Queries for the given `component_names` using latest-at semantics.
    ///
    /// See [`LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at(
        &self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> LatestAtResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = LatestAtResults::empty(query.clone());

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
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
                );

                let mut cache = cache.write();
                cache.handle_pending_invalidation();
                if let Some(cached) =
                    cache.latest_at(store, query, &clear_entity_path, ClearIsRecursive::name())
                {
                    let found_recursive_clear = cached.component_mono::<ClearIsRecursive>()
                        == Some(ClearIsRecursive(true.into()));
                    // When checking the entity itself, any kind of `Clear` component
                    // (i.e. recursive or not) will do.
                    //
                    // For (recursive) parents, we need to deserialize the data to make sure the
                    // recursive flag is set.
                    #[allow(clippy::collapsible_if)] // readability
                    if clear_entity_path == *entity_path || found_recursive_clear {
                        if let Some(index) = cached.index(&query.timeline()) {
                            if compare_indices(index, max_clear_index)
                                == std::cmp::Ordering::Greater
                            {
                                max_clear_index = index;
                            }
                        }
                    }
                }

                let Some(parent_entity_path) = clear_entity_path.parent() else {
                    break;
                };

                clear_entity_path = parent_entity_path;
            }
        }

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = if crate::cacheable(component_name) {
                Arc::clone(
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
                )
            } else {
                // If the component shouldn't be cached, simply instantiate a new cache for it.
                // It will be dropped when the user is done with it.
                Arc::new(RwLock::new(LatestAtCache::new(key.clone())))
            };

            let mut cache = cache.write();
            cache.handle_pending_invalidation();
            if let Some(cached) = cache.latest_at(store, query, entity_path, component_name) {
                // 1. A `Clear` component doesn't shadow its own self.
                // 2. If a `Clear` component was found with an index greater than or equal to the
                //    component data, then we know for sure that it should shadow it.
                if let Some(index) = cached.index(&query.timeline()) {
                    if component_name == ClearIsRecursive::name()
                        || compare_indices(index, max_clear_index) == std::cmp::Ordering::Greater
                    {
                        results.add(component_name, index, cached);
                    }
                }
            }
        }

        results
    }
}

// ---

/// Caches the results of `LatestAt` queries for a given [`CacheKey`].
pub struct LatestAtCache {
    /// For debugging purposes.
    pub cache_key: CacheKey,

    // TODO
    pub chunks: HashMap<ChunkId, Chunk>,

    /// These timestamps have been invalidated asynchronously.
    ///
    /// The next time this cache gets queried, it must remove any invalidated entries accordingly.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<ChunkId>,
}

impl LatestAtCache {
    #[inline]
    pub fn new(cache_key: CacheKey) -> Self {
        Self {
            cache_key,
            chunks: HashMap::default(),
            // cache: ChunkStore::new(
            //     re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            //     ChunkStoreConfig::ALL_DISABLED,
            // ),
            pending_invalidations: Default::default(),
        }
    }
}

impl std::fmt::Debug for LatestAtCache {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.chunks.fmt(f) // TODO
    }
}

impl SizeBytes for LatestAtCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key: _,
            chunks,
            pending_invalidations,
        } = self;

        let chunks = chunks.total_size_bytes();
        let pending_invalidations = pending_invalidations.total_size_bytes();

        chunks + pending_invalidations
    }
}

impl LatestAtCache {
    /// Queries cached latest-at data for a single component.
    pub fn latest_at(
        &mut self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Option<ChunkSharedMono> {
        re_tracing::profile_scope!("latest_at", format!("{query:?}"));

        // TODO: how the hell would we know whether something is missing, though?
        // I guess we could always fetch all the relevant chunks from the real store, which is
        // quite cheap anyhow, and then check whether we already have them sorted... which the does
        // on its own anyhow, right?
        //
        // or maybe we literally just keep a map of chunks to chunks, but pre-processed?

        let raw_chunks = store.latest_at_relevant_chunks(query, entity_path, component_name);
        for raw_chunk in &raw_chunks {
            self.chunks
                .entry(raw_chunk.id())
                .or_insert_with(|| raw_chunk.sorted_by_timeline_if_unsorted(&query.timeline()));
        }

        raw_chunks
            .into_iter()
            .filter_map(|raw_chunk| self.chunks.get(&raw_chunk.id()))
            .filter_map(|cached_sorted_chunk| {
                debug_assert!(cached_sorted_chunk.is_timeline_sorted(&query.timeline()));
                cached_sorted_chunk
                    .latest_at(query, component_name)
                    .into_mono()
                    .and_then(|chunk| chunk.index(&query.timeline()).map(|index| (index, chunk)))
            })
            .max_by_key(|(index, _chunk)| *index)
            .map(|(_index, chunk)| chunk)
    }

    #[inline]
    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            chunks,
            pending_invalidations,
        } = self;

        chunks.retain(|chunk_id, _chunk| !pending_invalidations.contains(&chunk_id));
    }
}
