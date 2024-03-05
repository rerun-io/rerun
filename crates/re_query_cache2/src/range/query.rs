use std::{
    cell::OnceCell,
    collections::{BTreeMap, VecDeque},
    sync::{Arc, OnceLock},
};

use ahash::{HashMap, HashSet};
use itertools::{izip, Either};
use nohash_hasher::{IntMap, IntSet};
use parking_lot::RwLock;

use re_data_store::{
    DataStore, LatestAtQuery, RangeQuery, StoreDiff, StoreEvent, StoreSubscriber, TimeInt,
};
use re_log_types::{DataCell, EntityPath, RowId, StoreId, Timeline};
use re_query2::QueryError;
use re_types_core::{components::InstanceKey, Archetype, Component, SizeBytes};
use re_types_core::{ComponentName, DeserializationError, DeserializationResult};

use crate::{
    CacheKey, CachedRangeComponentResults, CachedRangeResults, Caches, ErasedFlatVecDeque,
    FlatVecDeque,
};

// ---

// TODO: at some point we gotta be typed, there's no escaping it

impl Caches {
    pub fn range(
        &self,
        store: &DataStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: impl IntoIterator<Item = ComponentName>,
    ) -> CachedRangeResults {
        re_tracing::profile_function!(entity_path.to_string());

        let mut results = CachedRangeResults::default();

        for component_name in component_names {
            let key = CacheKey::new(entity_path.clone(), query.timeline, component_name);

            let cache = {
                let cache: Arc<RwLock<RangeCache>> = Arc::clone(
                    self.per_cache_key_range
                        .write()
                        .entry(key.clone())
                        .or_default(),
                );
                // Implicitly releasing top-level cache mappings -- concurrent queries can run once again.

                //TODO
                // let removed_bytes = caches_per_archetype.write().handle_pending_invalidation();
                // Implicitly releasing archetype-level cache mappings -- concurrent queries using the
                // same `CacheKey` but a different `ArchetypeName` can run once again.
                // if removed_bytes > 0 {
                //     re_log::trace!(
                //         store_id=%self.store_id,
                //         entity_path = %key.entity_path,
                //         removed = removed_bytes,
                //         "invalidated latest-at caches"
                //     );
                // }

                // let caches_per_archetype = caches_per_archetype.read();
                // let mut latest_at_per_archetype =
                //     caches_per_archetype.latest_at_per_archetype.write();
                // Arc::clone(latest_at_per_archetype.entry(A::name()).or_default())
                // // Implicitly releasing bottom-level cache mappings -- identical concurrent queries
                // // can run once again.

                cache
            };

            let mut cache = cache.write();
            // cache.handle_pending_invalidation(); // TODO
            // TODO
            cache.range(store, query, entity_path, component_name);
            // if let Some(cached) = cache.range(store, query, entity_path, component_name) {
            //     results.add(component_name, cached);
            // }
        }

        results
    }
}

// ---

// TODO: that's once again component based!

/// Caches the results of `Range` queries.
#[derive(Default)]
pub struct RangeCache {
    /// All timeful data, organized by _data_ time.
    ///
    /// Query time is irrelevant for range queries.
    //
    // TODO(#4810): bucketize
    pub per_data_time: Arc<RwLock<CachedRangeComponentResults>>,

    /// All timeless data.
    pub timeless: Arc<RwLock<CachedRangeComponentResults>>,

    /// For debugging purposes.
    pub(crate) timeline: Timeline,
}

impl RangeCache {
    pub fn range(
        &mut self,
        store: &DataStore,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Arc<RwLock<CachedRangeComponentResults>> {
        re_tracing::profile_scope!("range", format!("{query:?}"));

        // TODO

        Arc::new(RwLock::new(CachedRangeComponentResults::new()))
    }
}
