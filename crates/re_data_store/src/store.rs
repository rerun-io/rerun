use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::AtomicU64;

use arrow2::datatypes::DataType;
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_log_types::{
    DataCell, DataCellColumn, EntityPath, EntityPathHash, ErasedTimeVec, RowId, RowIdVec, StoreId,
    TimeInt, TimePoint, TimeRange, Timeline,
};
use re_types_core::{ComponentName, ComponentNameSet, SizeBytes};

// --- Data store ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStoreConfig {
    /// The maximum number of rows in an indexed bucket before triggering a split.
    /// Does not apply to static data.
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
        indexed_bucket_num_rows: 512,
        store_insert_ids: cfg!(debug_assertions),
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
pub struct DataStore {
    pub(crate) id: StoreId,

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
    pub(crate) metadata_registry: MetadataRegistry<(TimePoint, EntityPathHash)>,

    /// All temporal [`IndexedTable`]s for all entities on all timelines.
    ///
    /// See also [`Self::static_tables`].
    pub(crate) tables: BTreeMap<(EntityPathHash, Timeline), IndexedTable>,

    /// Static data. Never garbage collected.
    ///
    /// Static data unconditionally shadows temporal data at query time.
    ///
    /// Existing temporal will not be removed. Events won't be fired.
    ///
    /// See also [`Self::tables`].
    pub(crate) static_tables: BTreeMap<EntityPathHash, StaticTable>,

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
            config: self.config.clone(),
            type_registry: self.type_registry.clone(),
            metadata_registry: self.metadata_registry.clone(),
            tables: self.tables.clone(),
            static_tables: self.static_tables.clone(),
            insert_id: Default::default(),
            query_id: Default::default(),
            gc_id: Default::default(),
            event_id: Default::default(),
        }
    }
}

impl DataStore {
    pub fn new(id: StoreId, config: DataStoreConfig) -> Self {
        Self {
            id,
            config,
            type_registry: Default::default(),
            metadata_registry: Default::default(),
            tables: Default::default(),
            static_tables: Default::default(),
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
    /// Ignores static data.
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
                if let Some(&time) = bucket.inner.read().col_time.front() {
                    *entry = TimeInt::min(*entry, TimeInt::new_temporal(time));
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
    ) -> impl ExactSizeIterator<Item = ((EntityPath, Timeline), &IndexedTable)> {
        self.tables.iter().map(|((_, timeline), table)| {
            ((table.entity_path.clone() /* shallow */, *timeline), table)
        })
    }
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
/// cargo test -p re_data_store -- --nocapture datastore_internal_repr
/// ```
#[derive(Debug, Clone)]
pub struct IndexedTable {
    /// The timeline this table operates in, for debugging purposes.
    pub timeline: Timeline,

    /// The entity this table is related to, for debugging purposes.
    pub entity_path: EntityPath,

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
    pub fn new(timeline: Timeline, entity_path: EntityPath) -> Self {
        let bucket = IndexedBucket::new(timeline);
        let buckets_size_bytes = bucket.total_size_bytes();
        Self {
            timeline,
            entity_path,
            buckets: [(TimeInt::MIN, bucket)].into(),
            all_components: Default::default(),
            buckets_num_rows: 0,
            buckets_size_bytes,
        }
    }

    /// Makes sure bucketing invariants are upheld, and takes necessary actions if not.
    ///
    /// Invariants are:
    /// 1. There must always be at least one bucket alive.
    /// 2. The first bucket must always have an _indexing time_ `-∞`.
    pub(crate) fn uphold_indexing_invariants(&mut self) {
        if self.buckets.is_empty() {
            let Self {
                timeline,
                entity_path: _,
                buckets,
                all_components: _, // keep the history on purpose
                buckets_num_rows,
                buckets_size_bytes,
            } = self;

            let bucket = IndexedBucket::new(*timeline);
            let size_bytes = bucket.total_size_bytes();

            *buckets = [(TimeInt::MIN, bucket)].into();
            *buckets_num_rows = 0;
            *buckets_size_bytes = size_bytes;
        }
        // NOTE: Make sure the first bucket is responsible for `-∞`, which might or might not be
        // the case now if we've been moving buckets around.
        else if let Some((_, bucket)) = self.buckets.pop_first() {
            self.buckets.insert(TimeInt::MIN, bucket);
        }
    }
}

/// An `IndexedBucket` holds a chunk of rows from an [`IndexedTable`]
/// (see [`DataStoreConfig::indexed_bucket_num_rows`]).
#[derive(Debug)]
pub struct IndexedBucket {
    /// The timeline the bucket's parent table operates in, for debugging purposes.
    pub timeline: Timeline,

    // To simplify interior mutability.
    pub inner: RwLock<IndexedBucketInner>,
}

impl Clone for IndexedBucket {
    fn clone(&self) -> Self {
        Self {
            timeline: self.timeline,
            inner: RwLock::new(self.inner.read().clone()),
        }
    }
}

impl IndexedBucket {
    pub(crate) fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            inner: RwLock::new(IndexedBucketInner::default()),
        }
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

    /// Keeps track of the latest/newest [`RowId`] present in this bucket.
    ///
    /// Useful to batch GC buckets.
    ///
    /// `RowId::ZERO` for empty buckets.
    pub max_row_id: RowId,

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
            max_row_id: RowId::ZERO,
            columns: Default::default(),
            size_bytes: 0, // NOTE: computed below
        };
        this.compute_size_bytes();
        this
    }
}

// --- Static ---

/// Keeps track of static component data per entity.
#[derive(Clone)]
pub struct StaticTable {
    /// The entity this table is related to, for debugging purposes.
    pub entity_path: EntityPath,

    /// Keeps track of one and only one [`StaticCell`] per component.
    ///
    /// Last-write-wins semantics apply, where ordering is defined by `RowId`.
    pub cells: BTreeMap<ComponentName, StaticCell>,
}

impl StaticTable {
    #[inline]
    pub fn new(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            cells: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct StaticCell {
    /// None if [`DataStoreConfig::store_insert_ids`] is `false`.
    pub insert_id: Option<u64>,

    pub row_id: RowId,
    pub cell: DataCell,
}
