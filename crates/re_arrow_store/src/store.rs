use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::AtomicU64;

use ahash::HashMap;
use arrow2::datatypes::DataType;
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use smallvec::SmallVec;

use re_log_types::{
    external::re_tuid::Tuid, DataCell, DataCellColumn, EntityPath, EntityPathHash, ErasedTimeVec,
    NumInstancesVec, RowId, RowIdVec, StoreId, TableId, TimeInt, TimePoint, TimeRange, Timeline,
};
use re_types_core::{ComponentName, ComponentNameSet, SizeBytes};

// --- Data store ---

// TODO: time budget
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStoreConfig {
    /// The maximum number of rows in an indexed bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that indexed tables are always scoped
    /// to a specific timeline _and_ a specific entity.
    ///
    /// This effectively puts an upper bound on the number of rows that need to be sorted when an
    /// indexed bucket gets out of order (e.g. because of new insertions or a GC pass).
    /// This is a tradeoff: less rows means faster sorts at the cost of more metadata overhead.
    /// In particular:
    /// - Query performance scales inversely logarithmically to this number (i.e. it gets better
    ///   the higher this number gets).
    /// - GC performance scales quadratically with this number (i.e. it gets better the lower this
    ///   number gets).
    ///
    /// See [`Self::DEFAULT`] for defaults.
    pub indexed_bucket_num_rows: u64,

    /// If enabled, will store the ID of the write request alongside the inserted data.
    ///
    /// This can make inspecting the data within the store much easier, at the cost of an extra
    /// `u64` value stored per row.
    ///
    /// Enabled by default in debug builds.
    pub store_insert_ids: bool,

    /// If enabled, the store will throw an error if and when it notices that a single component
    /// type maps to more than one arrow datatype.
    ///
    /// Enabled by default in debug builds.
    pub enable_typecheck: bool,
}

impl Default for DataStoreConfig {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl DataStoreConfig {
    pub const DEFAULT: Self = Self {
        // NOTE: Empirical testing has shown that 512 is a good balance between sorting
        // and binary search costs with the current GC implementation.
        //
        // Garbage collection costs are entirely driven by the number of buckets around, the size
        // of the data itself has no impact.
        //
        // TODO: growing this number should help with GC perf... except it does the opposite???
        indexed_bucket_num_rows: 512,
        store_insert_ids: cfg!(debug_assertions),
        enable_typecheck: cfg!(debug_assertions),
    };
}

// ---

pub type InsertIdVec = VecDeque<u64>;

/// Keeps track of datatype information for all component types that have been written to the store
/// so far.
///
/// See also [`DataStore::lookup_datatype`].
#[derive(Debug, Default, Clone)]
pub struct DataTypeRegistry(pub IntMap<ComponentName, DataType>);

impl std::ops::Deref for DataTypeRegistry {
    type Target = IntMap<ComponentName, DataType>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for DataTypeRegistry {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Keeps track of arbitrary per-row metadata.
#[derive(Debug, Clone)]
pub struct MetadataRegistry<T: Clone> {
    pub registry: BTreeMap<RowId, T>,

    /// Cached heap size, because the registry gets very, very large.
    pub heap_size_bytes: u64,
}

impl Default for MetadataRegistry<(TimePoint, EntityPathHash)> {
    fn default() -> Self {
        let mut this = Self {
            registry: Default::default(),
            heap_size_bytes: 0,
        };
        this.heap_size_bytes = this.heap_size_bytes(); // likely zero, just future proofing
        this
    }
}

impl<T: Clone> std::ops::Deref for MetadataRegistry<T> {
    type Target = BTreeMap<RowId, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.registry
    }
}

impl<T: Clone> std::ops::DerefMut for MetadataRegistry<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.registry
    }
}

/// Used to cache auto-generated cluster cells (`[0]`, `[0, 1]`, `[0, 1, 2]`, …) so that they
/// can be properly deduplicated on insertion.
#[derive(Debug, Default, Clone)]
pub struct ClusterCellCache(pub IntMap<u32, DataCell>);

impl std::ops::Deref for ClusterCellCache {
    type Target = IntMap<u32, DataCell>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ClusterCellCache {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ---

/// Incremented on each edit.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StoreGeneration {
    insert_id: u64,
    gc_id: u64,
}

/// A complete data store: covers all timelines, all entities, everything.
///
/// ## Debugging
///
/// `DataStore` provides a very thorough `Display` implementation that makes it manageable to
/// know what's going on internally.
/// For even more information, you can set `RERUN_DATA_STORE_DISPLAY_SCHEMAS=1` in your
/// environment, which will result in additional schema information being printed out.
///
/// Additionally, if the `polars` feature is enabled, you can dump the entire datastore as a
/// flat denormalized dataframe using [`Self::to_dataframe`].
pub struct DataStore {
    pub(crate) id: StoreId,

    /// The cluster key specifies a column/component that is guaranteed to always be present for
    /// every single row of data within the store.
    ///
    /// In addition to always being present, the payload of the cluster key..:
    /// - is always increasingly sorted,
    /// - is always dense (no validity bitmap),
    /// - and never contains duplicate entries.
    ///
    /// This makes the cluster key a perfect candidate for joining query results together, and
    /// doing so as efficiently as possible.
    ///
    /// See [`Self::insert_row`] for more information.
    pub(crate) cluster_key: ComponentName,

    /// The configuration of the data store (e.g. bucket sizes).
    pub(crate) config: DataStoreConfig,

    /// Keeps track of datatype information for all component types that have been written to
    /// the store so far.
    ///
    /// See also [`Self::lookup_datatype`].
    //
    // TODO(#1809): replace this with a centralized Arrow registry.
    pub(crate) type_registry: DataTypeRegistry,

    /// Keeps track of arbitrary per-row metadata.
    ///
    /// Only used to map `RowId`s to their original [`TimePoint`]s at the moment.
    pub(crate) metadata_registry: MetadataRegistry<(TimePoint, EntityPathHash)>,

    /// Used to cache auto-generated cluster cells (`[0]`, `[0, 1]`, `[0, 1, 2]`, …)
    /// so that they can be properly deduplicated on insertion.
    pub(crate) cluster_cell_cache: ClusterCellCache,

    /// All temporal [`IndexedTable`]s for all entities on all timelines.
    ///
    /// See also [`Self::timeless_tables`].
    pub(crate) tables: HashMap<(Timeline, EntityPathHash), IndexedTable>,

    /// All timeless indexed tables for all entities. Never garbage collected.
    ///
    /// See also [`Self::tables`].
    pub(crate) timeless_tables: IntMap<EntityPathHash, PersistentIndexedTable>,

    /// Monotonically increasing ID for insertions.
    pub(crate) insert_id: u64,

    /// Monotonically increasing ID for queries.
    pub(crate) query_id: AtomicU64,

    /// Monotonically increasing ID for GCs.
    pub(crate) gc_id: u64,

    /// Monotonically increasing ID for store events.
    pub(crate) event_id: AtomicU64,
}

impl Clone for DataStore {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            cluster_key: self.cluster_key,
            config: self.config.clone(),
            type_registry: self.type_registry.clone(),
            metadata_registry: self.metadata_registry.clone(),
            cluster_cell_cache: self.cluster_cell_cache.clone(),
            tables: self.tables.clone(),
            timeless_tables: self.timeless_tables.clone(),
            insert_id: Default::default(),
            query_id: Default::default(),
            gc_id: Default::default(),
            event_id: Default::default(),
        }
    }
}

impl DataStore {
    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn new(id: StoreId, cluster_key: ComponentName, config: DataStoreConfig) -> Self {
        Self {
            id,
            cluster_key,
            config,
            cluster_cell_cache: Default::default(),
            type_registry: Default::default(),
            metadata_registry: Default::default(),
            tables: Default::default(),
            timeless_tables: Default::default(),
            insert_id: 0,
            query_id: AtomicU64::new(0),
            gc_id: 0,
            event_id: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn id(&self) -> &StoreId {
        &self.id
    }

    /// The column name used for storing insert requests' IDs alongside the data when manipulating
    /// dataframes.
    ///
    /// See [`DataStoreConfig::store_insert_ids`].
    pub fn insert_id_component_name() -> ComponentName {
        "rerun.controls.InsertId".into()
    }

    /// Return the current `StoreGeneration`. This can be used to determine whether the
    /// database has been modified since the last time it was queried.
    pub fn generation(&self) -> StoreGeneration {
        StoreGeneration {
            insert_id: self.insert_id,
            gc_id: self.gc_id,
        }
    }

    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn cluster_key(&self) -> ComponentName {
        self.cluster_key
    }

    /// See [`DataStoreConfig`] for more information about configuration.
    pub fn config(&self) -> &DataStoreConfig {
        &self.config
    }

    /// Lookup the arrow [`DataType`] of a [`re_types_core::Component`] in the internal
    /// `DataTypeRegistry`.
    pub fn lookup_datatype(&self, component: &ComponentName) -> Option<&DataType> {
        self.type_registry.get(component)
    }

    /// The oldest time for which we have any data.
    ///
    /// Ignores timeless data.
    ///
    /// Useful to call after a gc.
    pub fn oldest_time_per_timeline(&self) -> BTreeMap<Timeline, TimeInt> {
        re_tracing::profile_function!();

        let mut oldest_time_per_timeline = BTreeMap::default();

        for index in self.tables.values() {
            if let Some(bucket) = index.buckets.values().next() {
                let entry = oldest_time_per_timeline
                    .entry(bucket.timeline)
                    .or_insert(TimeInt::MAX);
                if let Some(time) = bucket.oldest_time() {
                    *entry = TimeInt::min(*entry, time);
                }
            }
        }

        oldest_time_per_timeline
    }

    /// Returns a read-only iterator over the raw indexed tables.
    ///
    /// Do _not_ use this to try and assert the internal state of the datastore.
    pub fn iter_indices(
        &self,
    ) -> impl ExactSizeIterator<Item = ((Timeline, EntityPath), &IndexedTable)> {
        self.tables.iter().map(|((timeline, _), table)| {
            ((*timeline, table.ent_path.clone() /* shallow */), table)
        })
    }
}

/// A simple example to look at the internal representation of a [`DataStore`].
///
/// Run with:
/// ```text
/// cargo test -p re_arrow_store -- --nocapture datastore_internal_repr
/// ```
#[test]
#[cfg(test)]
fn datastore_internal_repr() {
    use re_log_types::DataTable;
    use re_types_core::Loggable as _;

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        re_types::components::InstanceKey::name(),
        DataStoreConfig {
            indexed_bucket_num_rows: 0,
            store_insert_ids: true,
            enable_typecheck: true,
        },
    );

    let timeless = DataTable::example(true);
    eprintln!("{timeless}");
    for row in timeless.to_rows() {
        store.insert_row(&row.unwrap()).unwrap();
    }

    let temporal = DataTable::example(false);
    eprintln!("{temporal}");
    for row in temporal.to_rows() {
        store.insert_row(&row.unwrap()).unwrap();
    }

    store.sanity_check().unwrap();
    eprintln!("{store}");
}

// --- Temporal ---

/// An `IndexedTable` is an ever-growing, arbitrary large [`re_log_types::DataTable`] that is
/// optimized for time-based insertions and queries (which means a lot of bucketing).
///
/// See also [`IndexedBucket`].
///
/// Run the following command to display a visualization of the store's internal datastructures and
/// better understand how everything fits together:
/// ```text
/// cargo test -p re_arrow_store -- --nocapture datastore_internal_repr
/// ```
#[derive(Debug, Clone)]
pub struct IndexedTable {
    /// The timeline this table operates in, for debugging purposes.
    pub timeline: Timeline,

    /// The entity this table is related to, for debugging purposes.
    pub ent_path: EntityPath,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub cluster_key: ComponentName,

    /// The actual buckets, where the data is stored.
    ///
    /// The keys of this `BTreeMap` represent the lower bounds of the time-ranges covered by
    /// their associated buckets, _as seen from an indexing rather than a data standpoint_!
    ///
    /// This means that e.g. for the initial bucket, this will always be `-∞`, as from an
    /// indexing standpoint, all reads and writes with a time `t >= -∞` should go there, even
    /// though the bucket doesn't actually contains data with a timestamp of `-∞`!
    pub buckets: BTreeMap<TimeInt, IndexedBucket>,

    /// Track all of the components that have been written to.
    ///
    /// Note that this set will never be purged and will continue to return components that may
    /// have been set in the past even if all instances of that component have since been purged
    /// to free up space.
    pub all_components: ComponentNameSet,

    /// The number of rows stored in this table, across all of its buckets.
    pub buckets_num_rows: u64,

    /// The size of both the control & component data stored in this table, across all of its
    /// buckets, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, …).
    pub buckets_size_bytes: u64,
}

impl IndexedTable {
    pub fn new(cluster_key: ComponentName, timeline: Timeline, ent_path: EntityPath) -> Self {
        let bucket = IndexedBucket::new(cluster_key, timeline);
        let buckets_size_bytes = bucket.total_size_bytes();
        Self {
            timeline,
            ent_path,
            buckets: [(i64::MIN.into(), bucket)].into(),
            cluster_key,
            all_components: Default::default(),
            buckets_num_rows: 0,
            buckets_size_bytes,
        }
    }
}

/// An `IndexedBucket` holds a chunk of rows from an [`IndexedTable`]
/// (see [`DataStoreConfig::indexed_bucket_num_rows`]).
#[derive(Debug)]
pub struct IndexedBucket {
    // TODO
    // TODO: prob make sense to make a type for this...
    pub id: Tuid,

    /// The timeline the bucket's parent table operates in, for debugging purposes.
    pub timeline: Timeline,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub cluster_key: ComponentName,

    // To simplify interior mutability.
    pub inner: RwLock<IndexedBucketInner>,
}

impl Clone for IndexedBucket {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            timeline: self.timeline,
            cluster_key: self.cluster_key,
            inner: RwLock::new(self.inner.read().clone()),
        }
    }
}

impl IndexedBucket {
    fn new(cluster_key: ComponentName, timeline: Timeline) -> Self {
        Self {
            id: Tuid::random(),
            timeline,
            inner: RwLock::new(IndexedBucketInner::default()),
            cluster_key,
        }
    }

    fn oldest_time(&self) -> Option<TimeInt> {
        self.sort_indices_if_needed();
        self.inner.read().col_time.front().copied().map(Into::into)
    }
}

/// See [`IndexedBucket`]; this is a helper struct to simplify interior mutability.
#[derive(Debug, Clone)]
pub struct IndexedBucketInner {
    /// Are the rows in this table chunk sorted?
    ///
    /// Querying an [`IndexedBucket`] will always trigger a sort if the rows within aren't already
    /// sorted.
    pub is_sorted: bool,

    /// The time range covered by the primary time column (see [`Self::col_time`]).
    ///
    /// For an empty bucket, this defaults to `[+∞,-∞]`.
    pub time_range: TimeRange,

    // The primary time column, which is what drives the ordering of every other column.
    pub col_time: ErasedTimeVec,

    /// The entire column of insertion IDs, if enabled in [`DataStoreConfig`].
    ///
    /// Keeps track of insertion order from the point-of-view of the [`DataStore`].
    pub col_insert_id: InsertIdVec,

    /// The entire column of `RowId`s.
    ///
    /// Keeps track of the unique identifier for each row that was generated by the clients.
    pub col_row_id: RowIdVec,

    // TODO
    pub newest_row_id: RowId,

    /// The entire column of `num_instances`.
    ///
    /// Keeps track of the expected number of instances in each row.
    pub col_num_instances: NumInstancesVec,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component
    /// (i.e. the table is sparse).
    pub columns: IntMap<ComponentName, DataCellColumn>,

    /// The size of both the control & component data stored in this bucket, heap and stack
    /// included, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, …).
    ///
    /// We cache this because there can be many, many buckets.
    pub size_bytes: u64,
}

impl Default for IndexedBucketInner {
    fn default() -> Self {
        let mut this = Self {
            is_sorted: true,
            time_range: TimeRange::EMPTY,
            col_time: Default::default(),
            col_insert_id: Default::default(),
            col_row_id: Default::default(),
            newest_row_id: RowId::ZERO,
            col_num_instances: Default::default(),
            columns: Default::default(),
            size_bytes: 0, // NOTE: computed below
        };
        this.compute_size_bytes();
        this
    }
}

// --- Timeless ---

/// The timeless specialization of an [`IndexedTable`].
///
/// Run the following command to display a visualization of the store's internal datastructures and
/// better understand how everything fits together:
/// ```text
/// cargo test -p re_arrow_store -- --nocapture datastore_internal_repr
/// ```
//
// TODO(#1807): timeless should be row-id ordered too then
#[derive(Debug, Clone)]
pub struct PersistentIndexedTable {
    /// The entity this table is related to, for debugging purposes.
    pub ent_path: EntityPath,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub cluster_key: ComponentName,

    /// The entire column of insertion IDs, if enabled in [`DataStoreConfig`].
    ///
    /// Keeps track of insertion order from the point-of-view of the [`DataStore`].
    pub col_insert_id: InsertIdVec,

    /// The entire column of `RowId`s.
    ///
    /// Keeps track of the unique identifier for each row that was generated by the clients.
    pub col_row_id: RowIdVec,

    /// The entire column of `num_instances`.
    ///
    /// Keeps track of the expected number of instances in each row.
    pub col_num_instances: NumInstancesVec,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component
    /// (i.e. the table is sparse).
    pub columns: IntMap<ComponentName, DataCellColumn>,
}

impl PersistentIndexedTable {
    pub fn new(cluster_key: ComponentName, ent_path: EntityPath) -> Self {
        Self {
            cluster_key,
            ent_path,
            col_insert_id: Default::default(),
            col_row_id: Default::default(),
            col_num_instances: Default::default(),
            columns: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.col_num_instances.is_empty()
    }
}
