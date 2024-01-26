use std::{collections::BTreeMap, ops::Add, sync::Arc};

use ahash::HashMap;
use arrow2::datatypes::DataType;
use itertools::Itertools;
use nohash_hasher::IntMap;
use parking_lot::{Mutex, RwLock};

use re_log_types::{
    ApplicationId, DataCell, DataRow, DataTable, EntityPath, EntityPathHash, LogMsg, RowId,
    SetStoreInfo, StoreId, StoreInfo, StoreKind, TableId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{Component, ComponentName, ComponentNameSet, SizeBytes};

use crate::{
    DataStoreConfig, DataStoreStats, GarbageCollectionOptions, LatestAtQuery, RangeQuery,
    StoreEvent, StoreGeneration, StoreSubscriber, UnaryDataStore, VersionedComponent, WriteResult,
};

// ---

// TODO
#[derive(Clone)]
pub struct ShardedDataStore {
    pub(crate) id: StoreId,
    pub(crate) cluster_key: ComponentName,
    pub(crate) config: DataStoreConfig,
    pub(crate) shards: IntMap<EntityPathHash, Arc<RwLock<UnaryDataStore>>>,
}

impl ShardedDataStore {
    #[inline]
    fn for_all_shards<'a, F: FnMut(&UnaryDataStore) -> R, R>(
        &'a self,
        f: &'a mut F,
    ) -> impl ExactSizeIterator<Item = R> + 'a {
        self.shards.values().map(|store| f(&store.read()))
    }

    #[inline]
    fn get_shard(&self, entity_path_hash: EntityPathHash) -> Option<Arc<RwLock<UnaryDataStore>>> {
        self.shards
            .get(&entity_path_hash)
            .map(|shard| Arc::clone(shard))
    }

    #[inline]
    fn get_or_create_shard(
        &mut self,
        entity_path_hash: EntityPathHash,
    ) -> Arc<RwLock<UnaryDataStore>> {
        Arc::clone(self.shards.entry(entity_path_hash).or_insert_with(|| {
            Arc::new(RwLock::new(crate::UnaryDataStore::new(
                self.id.clone(),
                self.cluster_key,
                self.config.clone(),
            )))
        }))
    }
}

// TODO: bindings to store.rs
impl ShardedDataStore {
    #[inline]
    pub fn new(id: StoreId, cluster_key: ComponentName, config: DataStoreConfig) -> Self {
        Self {
            id,
            cluster_key,
            config,
            shards: Default::default(),
        }
    }

    #[inline]
    pub fn id(&self) -> &StoreId {
        &self.id
    }

    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn cluster_key(&self) -> ComponentName {
        self.cluster_key
    }

    /// Return the current `StoreGeneration`. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    #[inline]
    pub fn generation(&self) -> StoreGeneration {
        self.for_all_shards(&mut |store| store.generation())
            .reduce(StoreGeneration::max)
            .unwrap_or_default()
    }

    /// Lookup the arrow [`DataType`] of a [`re_types_core::Component`] in the internal
    /// `DataTypeRegistry`.
    #[inline]
    pub fn lookup_datatype(&self, component: &ComponentName) -> Option<DataType> {
        self.for_all_shards(&mut |store| store.lookup_datatype(component).cloned())
            .flatten()
            .next()
    }
}

// TODO: bindings to store_write.rs
impl ShardedDataStore {
    pub fn insert_table_with_retries(
        &mut self,
        mut table: DataTable,
        num_attempts: usize,
        step_size: u64,
    ) -> WriteResult<Vec<StoreEvent>> {
        re_tracing::profile_function!(format!("num_rows={}", table.num_rows()));

        // TODO(#1760): Compute the size of the datacells in the batching threads on the clients.
        table.compute_all_size_bytes();

        let mut rows_per_entity_path: HashMap<EntityPath, Vec<DataRow>> = Default::default();
        for row in table.to_rows() {
            let row = row?;
            rows_per_entity_path
                .entry(row.entity_path().clone())
                .or_default()
                .push(row);
        }

        let store_events = Arc::new(Mutex::new(Vec::with_capacity(table.num_rows() as _)));
        rayon::scope(|s| {
            for (entity_path, rows) in rows_per_entity_path {
                let store = self.get_or_create_shard(entity_path.hash());
                let store_events = Arc::clone(&store_events);

                s.spawn(move |_| {
                    let mut datastore = store.write();
                    let mut local_events = Vec::with_capacity(rows.len());
                    for row in rows {
                        // ## RowId duplication
                        //
                        // We shouldn't be attempting to retry in this instance: a duplicated RowId at this stage
                        // is likely a user error.
                        //
                        // We only do so because, the way our 'save' feature is currently implemented in the
                        // viewer can result in a single row's worth of data to be split across several insertions
                        // when loading that data back (because we dump per-bucket, and RowIds get duplicated
                        // across buckets).
                        //
                        // TODO(#1894): Remove this once the save/load process becomes RowId-driven.
                        local_events.push(
                            insert_row_with_retries(&mut datastore, row, num_attempts, step_size)
                                .unwrap(), // TODO
                        );
                    }

                    store_events.lock().extend(local_events);
                });
            }
        });

        let mut store_events = store_events.lock();
        Ok(std::mem::take(&mut store_events))
    }

    // TODO: don't call this in a loop
    pub fn insert_row(&mut self, row: &DataRow) -> WriteResult<StoreEvent> {
        let store = Arc::clone(
            self.shards
                .entry(row.entity_path().hash())
                .or_insert_with(|| {
                    Arc::new(RwLock::new(crate::UnaryDataStore::new(
                        self.id.clone(),
                        self.cluster_key,
                        self.config.clone(),
                    )))
                }),
        );

        let mut store = store.write();
        store.insert_row(row)
    }

    // TODO: don't call this in a loop
    pub fn insert_row_with_retries(
        &mut self,
        row: DataRow,
        num_attempts: usize,
        step_size: u64,
    ) -> WriteResult<StoreEvent> {
        let store = self.get_or_create_shard(row.entity_path().hash());
        let mut store = store.write();
        insert_row_with_retries(&mut store, row, num_attempts, step_size)
    }
}

// TODO: bindings to store_read.rs
impl ShardedDataStore {
    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// a specific [`Timeline`].
    ///
    /// # Temporal semantics
    ///
    /// In addition to the temporal results, this also includes all [`ComponentName`]s present in
    /// the timeless tables for this entity.
    pub fn all_components(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<Vec<ComponentName>> {
        let store = self.get_shard(entity_path.hash())?;
        let store = store.read();
        store.all_components(timeline, entity_path)
    }

    /// Queries the datastore for the cells of the specified `components`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s (as well as the associated _data_ time and `RowId`) on
    /// success.
    /// Success is defined by one thing and one thing only: whether a cell could be found for the
    /// `primary` component.
    /// The presence or absence of secondary components has no effect on the success criteria.
    ///
    /// # Temporal semantics
    ///
    /// Temporal indices take precedence, then timeless tables are queried to fill the holes left
    /// by missing temporal data.
    pub fn latest_at<const N: usize>(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(Option<TimeInt>, RowId, [Option<DataCell>; N])> {
        let store = self.get_shard(entity_path.hash())?;
        let store = store.read();
        store.latest_at(query, entity_path, primary, components)
    }

    /// Iterates the datastore in order to return the cells of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component, for the given time
    /// range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `components`, or `None` if
    /// said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    ///
    /// ⚠ Contrary to latest-at queries, range queries can and will yield multiple rows for a
    /// single timestamp if that timestamp happens to hold multiple entries for the `primary`
    /// component.
    /// On the contrary, they won't yield any rows that don't contain an actual value for the
    /// `primary` component, _even if said rows do contain a value for one the secondaries_!
    ///
    /// # Temporal semantics
    ///
    /// Yields the contents of the temporal indices.
    /// Iff the query's time range starts at `TimeInt::MIN`, this will yield the contents of the
    /// timeless tables before anything else.
    ///
    /// When yielding timeless entries, the associated time will be `None`.
    pub fn range<'a, const N: usize>(
        &'a self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (Option<TimeInt>, RowId, [Option<DataCell>; N])> + 'a {
        let store = self.get_shard(entity_path.hash()).unwrap(); // TODO
        let store = store.read();
        store
            .range(query, entity_path, components)
            // TODO: lul
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will generate a log message of `level` otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log messages on failure.
    pub fn query_latest_component_with_log_level<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
        level: re_log::Level,
    ) -> Option<VersionedComponent<C>> {
        let store = self.get_shard(entity_path.hash())?;
        let store = store.read();
        store.query_latest_component_with_log_level(entity_path, query, level)
    }

    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    #[inline]
    pub fn query_latest_component<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<VersionedComponent<C>> {
        self.query_latest_component_with_log_level(entity_path, query, re_log::Level::Warn)
    }

    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will return None and log a debug message otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely logs debug messages on failure.
    #[inline]
    pub fn query_latest_component_quiet<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<VersionedComponent<C>> {
        self.query_latest_component_with_log_level(entity_path, query, re_log::Level::Debug)
    }

    /// Call [`Self::query_latest_component`] at the given path, walking up the hierarchy until an instance is found.
    #[inline]
    pub fn query_latest_component_at_closest_ancestor<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(EntityPath, VersionedComponent<C>)> {
        re_tracing::profile_function!();

        let mut cur_path = Some(entity_path.clone());
        while let Some(path) = cur_path {
            if let Some(vc) = self.query_latest_component::<C>(&path, query) {
                return Some((path, vc));
            }
            cur_path = path.parent();
        }
        None
    }

    /// Get the latest value for a given [`re_types_core::Component`] and the associated [`RowId`],
    /// assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_timeless_component<C: Component>(
        &self,
        entity_path: &EntityPath,
    ) -> Option<VersionedComponent<C>> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());
        self.query_latest_component(entity_path, &query).map(|vc| {
            debug_assert!(vc.data_time.is_none());
            vc
        })
    }

    /// Get the latest value for a given [`re_types_core::Component`] and the associated [`RowId`],
    /// assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will return None and log a debug message otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log debug on failure.
    pub fn query_timeless_component_quiet<C: Component>(
        &self,
        entity_path: &EntityPath,
    ) -> Option<VersionedComponent<C>> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());
        self.query_latest_component_quiet(entity_path, &query)
            .map(|vc| {
                debug_assert!(vc.data_time.is_none());
                vc
            })
    }
}

// TODO: bindings to store_stats.rs
impl ShardedDataStore {
    #[inline]
    pub fn stats(&self) -> DataStoreStats {
        self.for_all_shards(&mut DataStoreStats::from_store)
            .reduce(DataStoreStats::add)
            .unwrap_or_default()
    }

    /// Returns the number of timeless index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its timeless indexed tables.
    #[inline]
    pub fn num_timeless_rows(&self) -> u64 {
        re_tracing::profile_function!();
        self.for_all_shards(&mut |store| store.num_timeless_rows())
            .sum::<u64>()
    }

    /// Returns the number of temporal index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its temporal indexed tables.
    #[inline]
    pub fn num_temporal_rows(&self) -> u64 {
        re_tracing::profile_function!();
        self.for_all_shards(&mut |store| store.num_temporal_rows())
            .sum::<u64>()
    }
}

// ---

// TODO: uuuuuuuuuuuuuuh

/// Inserts a [`DataRow`] into the [`DataStore`], retrying in case of duplicated `RowId`s.
///
/// Retries a maximum of `num_attempts` times if the row couldn't be inserted because of a
/// duplicated [`RowId`], bumping the [`RowId`]'s internal counter by a random number
/// (up to `step_size`) between attempts.
///
/// Returns the actual [`DataRow`] that was successfully inserted, if any.
///
/// The default value of `num_attempts` (see [`MAX_INSERT_ROW_ATTEMPTS`]) should be (way) more than
/// enough for all valid use cases.
///
/// When using this function, please add a comment explaining the rationale.
fn insert_row_with_retries(
    store: &mut UnaryDataStore,
    mut row: DataRow,
    num_attempts: usize,
    step_size: u64,
) -> crate::WriteResult<StoreEvent> {
    fn random_u64() -> u64 {
        let mut bytes = [0_u8; 8];
        getrandom::getrandom(&mut bytes).map_or(0, |_| u64::from_le_bytes(bytes))
    }

    for i in 0..num_attempts {
        match store.insert_row(&row) {
            Ok(event) => return Ok(event),
            Err(crate::WriteError::ReusedRowId(_)) => {
                // TODO(#1894): currently we produce duplicate row-ids when hitting the "save" button.
                // This means we hit this code path when loading an .rrd file that was saved from the viewer.
                // In the future a row-id clash should probably either be considered an error (with a loud warning)
                // or an ignored idempotent operation (with the assumption that if the RowId is the same, so is the data).
                // In any case, we cannot log loudly here.
                // We also get here because of `ClearCascade`, but that could be solved by adding a random increment
                // in `on_clear_cascade` (see https://github.com/rerun-io/rerun/issues/4469).
                re_log::trace!(
                    "Found duplicated RowId ({}) during insert. Incrementing it by random offset (retry {}/{})…",
                    row.row_id,
                    i + 1,
                    num_attempts
                );
                row.row_id = row.row_id.incremented_by(random_u64() % step_size + 1);
            }
            Err(err) => return Err(err),
        }
    }

    Err(crate::WriteError::ReusedRowId(row.row_id()))
}
