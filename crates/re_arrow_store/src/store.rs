use std::collections::BTreeMap;
use std::sync::atomic::AtomicU64;

use ahash::HashMap;
use arrow2::array::Int64Array;
use arrow2::datatypes::{DataType, TimeUnit};
use smallvec::SmallVec;

use nohash_hasher::{IntMap, IntSet};
use parking_lot::RwLock;
use re_log_types::{
    ComponentName, DataCell, DataCellColumn, EntityPath, EntityPathHash, ErasedTimeVec,
    NumInstancesVec, RowId, RowIdVec, TimeInt, TimePoint, TimeRange, Timeline,
};

// TODO(cmc): Do a big pass for superfluous DataCell allocations all over the place.

// --- Data store ---

#[derive(Debug, Clone)]
pub struct DataStoreConfig {
    /// The maximum number of rows in an indexed bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that indexed tables are always scoped
    /// to a specific timeline _and_ a specific entity.
    ///
    /// This effectively puts an upper bound on the number of rows that need to be sorted when an
    /// indexed bucket gets out of order.
    /// This is a tradeoff: less rows means faster sorts at the cost of more metadata overhead.
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
        indexed_bucket_num_rows: 1024,
        store_insert_ids: cfg!(debug_assertions),
        enable_typecheck: cfg!(debug_assertions),
    };
}

// ---

pub type InsertIdVec = SmallVec<[u64; 4]>;

/// Keeps track of datatype information for all component types that have been written to the store
/// so far.
///
/// See also [`DataStore::lookup_datatype`].
#[derive(Default)]
pub struct DataTypeRegistry(IntMap<ComponentName, DataType>);

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
#[derive(Default)]
pub struct MetadataRegistry<T: Clone>(HashMap<RowId, T>);

impl<T: Clone> std::ops::Deref for MetadataRegistry<T> {
    type Target = HashMap<RowId, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Clone> std::ops::DerefMut for MetadataRegistry<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
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
    pub cluster_key: ComponentName,

    /// The configuration of the data store (e.g. bucket sizes).
    pub config: DataStoreConfig,

    /// Keeps track of datatype information for all component types that have been written to
    /// the store so far.
    ///
    /// See also [`Self::lookup_datatype`].
    pub type_registry: DataTypeRegistry,

    /// Keeps track of arbitrary per-row metadata.
    ///
    /// Only used to map `RowId`s to their original [`TimePoint`]s at the moment.
    pub metadata_registry: MetadataRegistry<TimePoint>,

    /// Used to cache auto-generated cluster cells (`[0]`, `[0, 1]`, `[0, 1, 2]`, ...)
    /// so that they can be properly deduplicated on insertion.
    pub cluster_cell_cache: IntMap<u32, DataCell>,

    /// All temporal [`IndexedTable`]s for all entities on all timelines.
    ///
    /// See also [`Self::timeless_tables`].
    pub tables: HashMap<(Timeline, EntityPathHash), IndexedTable>,

    /// All timeless indexed tables for all entities. Never garbage collected.
    ///
    /// See also [`Self::tables`].
    pub timeless_tables: IntMap<EntityPathHash, PersistentIndexedTable>,

    /// Monotonically increasing ID for insertions.
    pub insert_id: u64,

    /// Monotonically increasing ID for queries.
    pub query_id: AtomicU64,

    /// Monotonically increasing ID for GCs.
    #[allow(dead_code)]
    pub gc_id: u64,
}

impl DataStore {
    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn new(cluster_key: ComponentName, config: DataStoreConfig) -> Self {
        Self {
            cluster_key,
            config,
            cluster_cell_cache: Default::default(),
            metadata_registry: Default::default(),
            type_registry: Default::default(),
            tables: Default::default(),
            timeless_tables: Default::default(),
            insert_id: 0,
            query_id: AtomicU64::new(0),
            gc_id: 0,
        }
    }

    /// The column name used for storing insert requests' IDs alongside the data when manipulating
    /// dataframes.
    ///
    /// See [`DataStoreConfig::store_insert_ids`].
    pub fn insert_id_key() -> ComponentName {
        "rerun.insert_id".into()
    }

    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn cluster_key(&self) -> ComponentName {
        self.cluster_key
    }

    /// Lookup the arrow [`DataType`] of a [`re_log_types::Component`] in the internal
    /// [`DataTypeRegistry`].
    pub fn lookup_datatype(&self, component: &ComponentName) -> Option<&DataType> {
        self.type_registry.get(component)
    }

    /// The oldest time for which we have any data.
    ///
    /// Ignores timeless data.
    ///
    /// Useful to call after a gc.
    pub fn oldest_time_per_timeline(&self) -> BTreeMap<Timeline, TimeInt> {
        crate::profile_function!();

        let mut oldest_time_per_timeline = BTreeMap::default();

        for index in self.tables.values() {
            if let Some(bucket) = index.buckets.values().next() {
                let entry = oldest_time_per_timeline
                    .entry(bucket.timeline)
                    .or_insert(TimeInt::MAX);
                if let Some(time) = bucket.inner.read().col_time.first() {
                    *entry = TimeInt::min(*entry, (*time).into());
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

// --- Temporal ---

/// An `IndexedTable` is an ever-growing, arbitrary large [`re_log_types::DataTable`] that is
/// optimized for time-based insertions and queries (which means a lot of bucketing).
///
/// See also [`IndexedBucket`].
///
/// TODO
#[derive(Debug)]
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
    pub all_components: IntSet<ComponentName>,

    /// The total number of rows in this indexed table, accounting for all buckets.
    pub total_rows: u64,

    /// The size of this table in bytes across all of its buckets, accounting for both data and
    /// metadata.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    /// Also: there are many buckets.
    pub total_size_bytes: u64,
}

impl IndexedTable {
    pub fn new(cluster_key: ComponentName, timeline: Timeline, ent_path: EntityPath) -> Self {
        Self {
            timeline,
            ent_path,
            buckets: [(i64::MIN.into(), IndexedBucket::new(cluster_key, timeline))].into(),
            cluster_key,
            all_components: Default::default(),
            total_rows: 0,
            total_size_bytes: 0, // TODO
        }
    }

    /// Recomputes the size of the table from scratch.
    ///
    /// Beware: this is costly!
    pub fn compute_total_size_bytes(&mut self) -> u64 {
        let Self {
            timeline,
            ent_path,
            cluster_key,
            buckets,
            all_components,
            total_rows,
            total_size_bytes,
        } = self;

        let buckets_size_bytes = buckets
            .values_mut()
            .map(|bucket| bucket.compute_total_size_bytes())
            .sum::<u64>();

        let size_bytes = std::mem::size_of_val(timeline)
            + std::mem::size_of_val(ent_path)
            + std::mem::size_of_val(cluster_key)
            + (all_components.len() * std::mem::size_of::<ComponentName>())
            + std::mem::size_of_val(total_rows)
            + std::mem::size_of_val(total_size_bytes);

        *total_size_bytes = size_bytes as u64 + buckets_size_bytes;

        *total_size_bytes
    }

    /// Returns a read-only iterator over the raw buckets.
    ///
    /// Do _not_ use this to try and test the internal state of the datastore.
    #[doc(hidden)]
    pub fn iter_buckets(&self) -> impl ExactSizeIterator<Item = &IndexedBucket> {
        self.buckets.values()
    }
}

/// An `IndexedBucket` holds a chunk of rows from an [`IndexedTable`]
/// (see [`DataStoreConfig::indexed_bucket_num_rows`]).
#[derive(Debug)]
pub struct IndexedBucket {
    /// The timeline the bucket's parent table operates in, for debugging purposes.
    pub timeline: Timeline,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub cluster_key: ComponentName,

    // To simplify interior mutability.
    pub inner: RwLock<IndexedBucketInner>,
}

impl IndexedBucket {
    fn new(cluster_key: ComponentName, timeline: Timeline) -> Self {
        Self {
            timeline,
            inner: RwLock::new(IndexedBucketInner::default()),
            cluster_key,
        }
    }

    /// Recomputes the size of the bucket from scratch.
    ///
    /// Beware: this is costly!
    pub fn compute_total_size_bytes(&mut self) -> u64 {
        self.inner.write().compute_total_size_bytes()
    }
}

/// See [`IndexedBucket`]; this is a helper struct to simplify interior mutability.
#[derive(Debug)]
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

    /// The entire column of `num_instances`.
    ///
    /// Keeps track of the expected number of instances in each row.
    pub col_num_instances: NumInstancesVec,

    /// All the rows for all the component columns.
    ///
    /// The cells are optional since not all rows will have data for every single component
    /// (i.e. the table is sparse).
    pub columns: IntMap<ComponentName, DataCellColumn>,

    /// The size of this bucket in bytes, accounting for both data and metadata.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    pub total_size_bytes: u64,
}

impl Default for IndexedBucketInner {
    fn default() -> Self {
        Self {
            is_sorted: true,
            time_range: TimeRange::new(i64::MAX.into(), i64::MIN.into()),
            col_time: Default::default(),
            col_insert_id: Default::default(),
            col_row_id: Default::default(),
            col_num_instances: Default::default(),
            columns: Default::default(),
            total_size_bytes: std::mem::size_of::<Self>() as _,
        }
    }
}

impl IndexedBucket {
    /// Returns a (name, [`Int64Array`]) with a logical type matching the timeline.
    pub fn times(&self) -> (String, Int64Array) {
        crate::profile_function!();

        let times = Int64Array::from_slice(self.inner.read().col_time.as_slice());
        let logical_type = match self.timeline.typ() {
            re_log_types::TimeType::Time => DataType::Timestamp(TimeUnit::Nanosecond, None),
            re_log_types::TimeType::Sequence => DataType::Int64,
        };
        (self.timeline.name().to_string(), times.to(logical_type))
    }
}

impl IndexedBucketInner {
    /// Recomputes the size of the bucket from scratch.
    ///
    /// Beware: this is costly!
    pub fn compute_total_size_bytes(&mut self) -> u64 {
        let Self {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            total_size_bytes,
        } = self;

        let size_bytes = std::mem::size_of_val(is_sorted)
            + std::mem::size_of_val(time_range)
            + std::mem::size_of_val(col_time.as_slice())
            + std::mem::size_of_val(col_insert_id.as_slice())
            + std::mem::size_of_val(col_row_id.as_slice())
            + std::mem::size_of_val(col_num_instances.as_slice())
            + std::mem::size_of_val(total_size_bytes);

        *total_size_bytes = size_bytes as u64 + compute_table_size_bytes(columns);

        *total_size_bytes
    }
}

/// A simple example to look at the internal representation of the a [`DataStore`].
///
/// Run with:
/// ```text
/// cargo test -p re_arrow_store -- --nocapture datastore_docgen_indexed_table
/// ```
#[test]
fn datastore_docgen_indexed_table() {
    use re_log_types::{component_types::InstanceKey, Component as _, DataTable};

    let table = DataTable::example(false);

    let mut store = DataStore::new(
        InstanceKey::name(),
        DataStoreConfig {
            indexed_bucket_num_rows: 0,
            store_insert_ids: true,
            enable_typecheck: true,
        },
    );

    eprintln!("{table}");
    store.insert_table(&table).unwrap();

    store.sanity_check().unwrap();
    eprintln!("{store}");
}

// --- Timeless ---

/// The timeless specialization of an [`IndexedTable`].
///
/// TODO
#[derive(Debug)]
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

    /// The size of this indexed table in bytes, accounting for both data and metadata.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    pub total_size_bytes: u64,
}

impl PersistentIndexedTable {
    /// Recomputes the size of the table from scratch.
    ///
    /// Beware: this is costly!
    pub fn compute_total_size_bytes(&mut self) -> u64 {
        let Self {
            ent_path,
            cluster_key,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            total_size_bytes,
        } = self;

        let size_bytes = std::mem::size_of_val(ent_path)
            + std::mem::size_of_val(cluster_key)
            + std::mem::size_of_val(col_insert_id.as_slice())
            + std::mem::size_of_val(col_row_id.as_slice())
            + std::mem::size_of_val(col_num_instances.as_slice())
            + std::mem::size_of_val(total_size_bytes);

        *total_size_bytes = size_bytes as u64 + compute_table_size_bytes(columns);

        *total_size_bytes
    }

    pub fn is_empty(&self) -> bool {
        self.col_num_instances.is_empty()
    }
}

// --- Common ---

/// Computes the size in bytes of an entire table's worth of data.
///
/// Beware: this is costly!
fn compute_table_size_bytes(columns: &IntMap<ComponentName, DataCellColumn>) -> u64 {
    let keys = (columns.keys().len() * std::mem::size_of::<ComponentName>()) as u64;
    let cells = columns
        .values()
        .flat_map(|column| column.iter())
        .flatten() // option
        .map(|cell| cell.size_bytes())
        .sum::<u64>();
    keys + cells
}
