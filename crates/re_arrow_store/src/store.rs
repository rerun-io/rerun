use std::collections::{BTreeMap, HashMap, VecDeque};
use std::num::NonZeroU64;
use std::sync::atomic::AtomicU64;

use arrow2::array::{Array, Int64Array};
use arrow2::datatypes::{DataType, TimeUnit};

use nohash_hasher::{IntMap, IntSet};
use parking_lot::RwLock;
use re_log_types::{
    ComponentName, EntityPath, EntityPathHash, MsgId, TimeInt, TimePoint, TimeRange, Timeline,
};

// --- Indices & offsets ---

/// A vector of times. Our primary column, always densely filled.
pub type TimeIndex = Vec<i64>;

/// A vector of references into the component tables. None = null.
// TODO(cmc): keeping a separate validity might be a better option, maybe.
pub type SecondaryIndex = Vec<Option<RowIndex>>;
static_assertions::assert_eq_size!(u64, Option<RowIndex>);

// TODO(#639): We desperately need to work on the terminology here:
//
// - `TimeIndex` is a vector of `TimeInt`s.
//   It's the primary column and it's always dense.
//   It's used to search the datastore by time.
//
// - `ComponentIndex` (currently `SecondaryIndex`) is a vector of `ComponentRowNr`s.
//   It's the secondary column and is sparse.
//   It's used to search the datastore by component once the search by time is complete.
//
// - `ComponentRowNr` (currently `RowIndex`) is a row offset into a component table.
//   It only makes sense when associated with a component name.
//   It is absolute.
//   It's used to fetch actual data from the datastore.
//
// - `IndexRowNr` is a row offset into an index bucket.
//   It only makes sense when associated with an entity path and a specific time.
//   It is relative per bucket.
//   It's used to tiebreak results with an identical time, should you need too.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum RowIndexKind {
    Temporal = 0,
    Timeless = 1,
}

/// An opaque type that directly refers to a row of data within the datastore, iff it is
/// associated with a component name.
///
/// See [`DataStore::latest_at`], [`DataStore::range`] & [`DataStore::get`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowIndex(pub(crate) NonZeroU64);

impl RowIndex {
    const KIND_MASK: u64 = 0x8000_0000_0000_0000;

    /// Panics if `v` is 0.
    /// In debug, panics if `v` has its most significant bit set.
    pub(crate) fn from_u63(kind: RowIndexKind, v: u64) -> Self {
        debug_assert!(v & Self::KIND_MASK == 0);

        let v = v | ((kind as u64) << 63);
        Self(v.try_into().unwrap())
    }

    pub(crate) fn as_u64(self) -> u64 {
        self.0.get() & !Self::KIND_MASK
    }

    pub(crate) fn kind(self) -> RowIndexKind {
        match self.0.get() & Self::KIND_MASK > 0 {
            false => RowIndexKind::Temporal,
            true => RowIndexKind::Timeless,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexRowNr(pub(crate) u64);

// --- Data store ---

#[derive(Debug, Clone)]
pub struct DataStoreConfig {
    /// The maximum size of a component bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that component tables are shared
    /// across all timelines and all entities!
    ///
    /// This effectively controls how fine grained the garbage collection of components is.
    /// The lower the size, the more fine-grained the garbage collection is, at the cost of more
    /// metadata overhead.
    ///
    /// Note that this cannot split a single huge row: if a user inserts a single row that's
    /// larger than the threshold, then that bucket will become larger than the threshold, and
    /// we will split from there on.
    ///
    /// See [`Self::DEFAULT`] for defaults.
    pub component_bucket_size_bytes: u64,

    /// The maximum number of rows in a component bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that component tables are shared
    /// across all timelines and all entities!
    ///
    /// This effectively controls how fine grained the garbage collection of components is.
    /// The lower the number, the more fine-grained the garbage collection is, at the cost of more
    /// metadata overhead.
    ///
    /// Note: since component buckets aren't sorted, the number of rows isn't necessarily a great
    /// metric to use as a threshold, although we do expose it if only for symmetry.
    /// Prefer using [`Self::component_bucket_size_bytes`], or both.
    ///
    /// See [`Self::DEFAULT`] for defaults.
    pub component_bucket_nb_rows: u64,

    /// The maximum size of an index bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that index tables are always scoped
    /// to a specific timeline _and_ a specific entity.
    ///
    /// This effectively controls two aspects of the runtime:
    /// - how fine grained the garbage collection of indices is,
    /// - and how many rows will have to be sorted in the worst case when an index gets out
    ///   of order.
    /// The lower the size, the more fine-grained the garbage collection is and smaller the
    /// number of rows to sort gets, at the cost of more metadata overhead.
    ///
    /// See [`Self::DEFAULT`] for defaults.
    pub index_bucket_size_bytes: u64,

    /// The maximum number of rows in an index bucket before triggering a split.
    /// Does not apply to timeless data.
    ///
    /// ⚠ When configuring this threshold, do keep in mind that index tables are always scoped
    /// to a specific timeline _and_ a specific entity.
    ///
    /// This effectively controls two aspects of the runtime:
    /// - how fine grained the garbage collection of indices is,
    /// - and how many rows will have to be sorted in the worst case when an index gets out
    ///   of order.
    /// The lower the size, the more fine-grained the garbage collection is and smaller the
    /// number of rows to sort gets, at the cost of more metadata overhead.
    ///
    /// See [`Self::DEFAULT`] for defaults.
    pub index_bucket_nb_rows: u64,

    /// If enabled, will store the ID of the write request alongside the inserted data.
    ///
    /// This can make inspecting the data within the store much easier, at the cost of an extra
    /// `u64` value stored per row.
    ///
    /// Enabled by default in debug builds.
    ///
    /// See [`DataStore::insert_id_key`].
    pub store_insert_ids: bool,

    /// Should soon-to-be inactive buckets be compacted before being archived?
    pub enable_compaction: bool,
}

impl Default for DataStoreConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl DataStoreConfig {
    pub const DEFAULT: Self = Self {
        component_bucket_size_bytes: 32 * 1024 * 1024, // 32MiB
        component_bucket_nb_rows: u64::MAX,
        index_bucket_size_bytes: 32 * 1024, // 32kiB
        index_bucket_nb_rows: 1024,
        store_insert_ids: cfg!(debug_assertions),
        // TODO(cmc): Compaction is disabled until we implement batching.
        // See https://github.com/rerun-io/rerun/pull/1535 for rationale.
        //
        // This has no noticeable impact on performance.
        enable_compaction: false,
    };
}

// ---

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
    /// See [`Self::insert`] for more information.
    pub(crate) cluster_key: ComponentName,

    /// The configuration of the data store (e.g. bucket sizes).
    pub(crate) config: DataStoreConfig,

    /// Maps `MsgId`s to some metadata (just timepoints at the moment).
    ///
    /// `BTreeMap` because of garbage collection.
    pub(crate) messages: BTreeMap<MsgId, TimePoint>,

    /// Used to cache auto-generated cluster components, i.e. `[0]`, `[0, 1]`, `[0, 1, 2]`, etc
    /// so that they can be properly deduplicated.
    pub(crate) cluster_comp_cache: IntMap<usize, RowIndex>,

    /// Dedicated index tables for timeless data. Never garbage collected.
    ///
    /// See also `Self::indices`.
    pub(crate) timeless_indices: IntMap<EntityPathHash, PersistentIndexTable>,

    /// Dedicated component tables for timeless data. Never garbage collected.
    ///
    /// See also `Self::components`.
    pub(crate) timeless_components: IntMap<ComponentName, PersistentComponentTable>,

    /// Maps an entity to its index, for a specific timeline.
    ///
    /// An index maps specific points in time to rows in component tables.
    pub(crate) indices: HashMap<(Timeline, EntityPathHash), IndexTable>,

    /// Maps a component name to its associated table, for all timelines and all entities.
    ///
    /// A component table holds all the values ever inserted for a given component.
    pub(crate) components: IntMap<ComponentName, ComponentTable>,

    /// Monotonically increasing ID for insertions.
    pub(crate) insert_id: u64,

    /// Monotonically increasing ID for queries.
    pub(crate) query_id: AtomicU64,

    /// Monotonically increasing ID for GCs.
    pub(crate) gc_id: u64,
}

impl DataStore {
    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn new(cluster_key: ComponentName, config: DataStoreConfig) -> Self {
        Self {
            cluster_key,
            config,
            cluster_comp_cache: Default::default(),
            messages: Default::default(),
            indices: Default::default(),
            components: Default::default(),
            timeless_indices: Default::default(),
            timeless_components: Default::default(),
            insert_id: 0,
            query_id: AtomicU64::new(0),
            gc_id: 0,
        }
    }

    /// The column name used for storing insert requests' IDs alongside the data.
    ///
    /// The insert IDs are stored as-is directly into the index tables, this is _not_ an
    /// indirection into an associated component table!
    ///
    /// See [`DataStoreConfig::store_insert_ids`].
    pub fn insert_id_key() -> ComponentName {
        "rerun.insert_id".into()
    }

    /// See [`Self::cluster_key`] for more information about the cluster key.
    pub fn cluster_key(&self) -> ComponentName {
        self.cluster_key
    }

    /// Lookup the arrow `DataType` of a `Component`
    pub fn lookup_data_type(&self, component: &ComponentName) -> Option<&DataType> {
        self.components.get(component).map(|c| &c.datatype)
    }
}

// --- Persistent Indices ---

/// A `PersistentIndexTable` maps specific entries to rows in persistent component tables.
///
/// See also `DataStore::IndexTable`.
#[derive(Debug)]
pub struct PersistentIndexTable {
    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub(crate) cluster_key: ComponentName,

    /// The number of rows in the table: all indices should always be exactly of that length.
    pub(crate) num_rows: u64,

    /// All component indices for this bucket.
    ///
    /// One index per component: new components (and as such, new indices) can be added at any
    /// time!
    /// When that happens, they will be retro-filled with nulls until they are [`Self::num_rows`]
    /// long.
    pub(crate) indices: IntMap<ComponentName, SecondaryIndex>,

    /// Track all of the components that have been written to.
    pub(crate) all_components: IntSet<ComponentName>,
}

// --- Indices ---

/// An `IndexTable` maps specific points in time to rows in component tables.
///
/// Example of a time-based index table (`MAX_ROWS_PER_BUCKET=2`):
/// ```text
/// IndexTable {
///     timeline: log_time
///     entity: this/that
///     size: 3 buckets for a total of 152 B across 5 total rows
///     buckets: [
///         IndexBucket {
///             index time bound: >= +0.000s
///             size: 64 B across 2 rows
///                 - log_time: from 19:37:35.713798Z to 19:37:35.713798Z (all inclusive)
///             data (sorted=true):
///             +-------------------------------+--------------+---------------+--------------------+
///             | log_time                      | rerun.rect2d | rerun.point2d | rerun.instance_key |
///             +-------------------------------+--------------+---------------+--------------------+
///             | 2022-12-20 19:37:35.713798552 |              | 2             | 2                  |
///             | 2022-12-20 19:37:35.713798552 | 4            |               | 2                  |
///             +-------------------------------+--------------+---------------+--------------------+
///
///         }
///         IndexBucket {
///             index time bound: >= 19:37:36.713798Z
///             size: 64 B across 2 rows
///                 - log_time: from 19:37:36.713798Z to 19:37:36.713798Z (all inclusive)
///             data (sorted=true):
///             +-------------------------------+--------------+--------------------+---------------+
///             | log_time                      | rerun.rect2d | rerun.instance_key | rerun.point2d |
///             +-------------------------------+--------------+--------------------+---------------+
///             | 2022-12-20 19:37:36.713798552 | 1            | 2                  |               |
///             | 2022-12-20 19:37:36.713798552 |              | 4                  |               |
///             +-------------------------------+--------------+--------------------+---------------+
///
///         }
///         IndexBucket {
///             index time bound: >= 19:37:37.713798Z
///             size: 24 B across 1 rows
///                 - log_time: from 19:37:37.713798Z to 19:37:37.713798Z (all inclusive)
///             data (sorted=true):
///             +-------------------------------+--------------+--------------------+
///             | log_time                      | rerun.rect2d | rerun.instance_key |
///             +-------------------------------+--------------+--------------------+
///             | 2022-12-20 19:37:37.713798552 | 2            | 3                  |
///             +-------------------------------+--------------+--------------------+
///
///         }
///     ]
/// }
/// ```
///
/// Example of a sequence-based index table (`MAX_ROWS_PER_BUCKET=2`):
/// ```text
/// IndexTable {
///     timeline: frame_nr
///     entity: this/that
///     size: 3 buckets for a total of 256 B across 8 total rows
///     buckets: [
///         IndexBucket {
///             index time bound: >= #0
///             size: 96 B across 3 rows
///                 - frame_nr: from #41 to #41 (all inclusive)
///             data (sorted=true):
///             +----------+---------------+--------------+--------------------+
///             | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance_key |
///             +----------+---------------+--------------+--------------------+
///             | 41       |               |              | 1                  |
///             | 41       | 1             |              | 2                  |
///             | 41       |               | 3            | 2                  |
///             +----------+---------------+--------------+--------------------+
///
///         }
///         IndexBucket {
///             index time bound: >= #42
///             size: 96 B across 3 rows
///                 - frame_nr: from #42 to #42 (all inclusive)
///             data (sorted=true):
///             +----------+--------------+--------------------+---------------+
///             | frame_nr | rerun.rect2d | rerun.instance_key | rerun.point2d |
///             +----------+--------------+--------------------+---------------+
///             | 42       | 1            | 2                  |               |
///             | 42       |              | 4                  |               |
///             | 42       |              | 2                  | 2             |
///             +----------+--------------+--------------------+---------------+
///
///         }
///         IndexBucket {
///             index time bound: >= #43
///             size: 64 B across 2 rows
///                 - frame_nr: from #43 to #44 (all inclusive)
///             data (sorted=true):
///             +----------+--------------+---------------+--------------------+
///             | frame_nr | rerun.rect2d | rerun.point2d | rerun.instance_key |
///             +----------+--------------+---------------+--------------------+
///             | 43       | 4            |               | 2                  |
///             | 44       |              | 3             | 2                  |
///             +----------+--------------+---------------+--------------------+
///
///         }
///     ]
/// }
/// ```
///
/// See also: [`IndexBucket`].
#[derive(Debug)]
pub struct IndexTable {
    /// The timeline this table operates in, for debugging purposes.
    pub(crate) timeline: Timeline,

    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,

    /// The actual buckets, where the indices are stored.
    ///
    /// The keys of this `BTreeMap` represent the lower bounds of the time-ranges covered by
    /// their associated buckets, _as seen from an indexing rather than a data standpoint_!
    ///
    /// This means that e.g. for the initial bucket, this will always be `-∞`, as from an
    /// indexing standpoint, all reads and writes with a time `t >= -∞` should go there, even
    /// though the bucket doesn't actually contains data with a timestamp of `-∞`!
    pub(crate) buckets: BTreeMap<TimeInt, IndexBucket>,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub(crate) cluster_key: ComponentName,

    /// Track all of the components that have been written to.
    ///
    /// Note that this set will never be purged and will continue to return
    /// components that may have been set in the past even if all instances of
    /// that component have since been purged to free up space.
    pub(crate) all_components: IntSet<ComponentName>,
}

impl IndexTable {
    pub fn entity_path(&self) -> &EntityPath {
        &self.ent_path
    }
}

/// An `IndexBucket` holds a size-delimited (data size and/or number of rows) chunk of a
/// [`IndexTable`].
///
/// - The data size limit is for garbage collection purposes.
/// - The number of rows limit is to bound sorting costs on the read path.
///
/// See [`IndexTable`] to get an idea of what an `IndexBucket` looks like in practice.
#[derive(Debug)]
pub struct IndexBucket {
    /// The timeline the bucket's parent table operates in, for debugging purposes.
    pub(crate) timeline: Timeline,

    pub(crate) indices: RwLock<IndexBucketIndices>,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub(crate) cluster_key: ComponentName,
}

/// Just the indices, to simplify interior mutability.
#[derive(Debug)]
pub struct IndexBucketIndices {
    /// Whether the indices (all of them!) are currently sorted.
    ///
    /// Querying an `IndexBucket` will always trigger a sort if the indices aren't already sorted.
    pub(crate) is_sorted: bool,

    /// The time range covered by the primary time index.
    ///
    /// This is the actual time range that's covered by the indexed data!
    /// For an empty bucket, this defaults to [+∞,-∞].
    pub(crate) time_range: TimeRange,

    // The primary time index, which is guaranteed to be dense, and "drives" all other indices.
    //
    // All secondary indices are guaranteed to follow the same sort order and be the same length.
    pub(crate) times: TimeIndex,

    /// All secondary indices for this bucket (i.e. everything but time).
    ///
    /// One index per component: new components (and as such, new indices) can be added at any
    /// time!
    /// When that happens, they will be retro-filled with nulls so that they share the same
    /// length as the primary index ([`Self::times`]).
    pub(crate) indices: IntMap<ComponentName, SecondaryIndex>,
}

impl Default for IndexBucketIndices {
    fn default() -> Self {
        Self {
            is_sorted: true,
            time_range: TimeRange::new(i64::MAX.into(), i64::MIN.into()),
            times: Default::default(),
            indices: Default::default(),
        }
    }
}

impl IndexBucket {
    /// Returns an (name, [`Int64Array`]) with a logical type matching the timeline.
    pub fn times(&self) -> (String, Int64Array) {
        crate::profile_function!();

        let times = Int64Array::from_vec(self.indices.read().times.clone());
        let logical_type = match self.timeline.typ() {
            re_log_types::TimeType::Time => DataType::Timestamp(TimeUnit::Nanosecond, None),
            re_log_types::TimeType::Sequence => DataType::Int64,
        };
        (self.timeline.name().to_string(), times.to(logical_type))
    }
}

// --- Persistent Components ---

/// A `PersistentComponentTable` holds all the timeless values ever inserted for a given component.
///
/// See also `DataStore::ComponentTable`.
#[derive(Debug)]
pub struct PersistentComponentTable {
    /// Name of the underlying component, for debugging purposes.
    pub(crate) name: ComponentName,

    /// Type of the underlying component.
    pub(crate) datatype: DataType,

    /// All the data for this table: many rows of a single column.
    ///
    /// Each chunk is a list of arrays of structs, i.e. `ListArray<StructArray>`:
    /// - the list layer corresponds to the different rows,
    /// - the array layer corresponds to the different instances within a single row,
    /// - and finally the struct layer holds the components themselves.
    /// E.g.:
    /// ```text
    /// [
    ///   [{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}],
    ///   [{x: 7.158843, y: 0.68897724}, {x: 8.934421, y: 2.8420508}],
    /// ]
    /// ```
    ///
    /// This can contain any number of chunks, depending on how the data was inserted (e.g. single
    /// insertions vs. batches).
    ///
    /// Note that, as of today, we do not actually support batched insertion nor do we support
    /// chunks of non-unit length (batches are inserted on a per-row basis internally).
    /// As a result, chunks always contain one and only one row's worth of data, at least until
    /// the bucket is compacted one or more times.
    /// See also #589.
    //
    // TODO(cmc): compact timeless tables once in a while
    pub(crate) chunks: Vec<Box<dyn Array>>,

    /// The total number of rows present in this bucket, across all chunks.
    pub(crate) total_rows: u64,

    /// The size of this bucket in bytes, across all chunks.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    pub(crate) total_size_bytes: u64,
}

// --- Components ---

/// A `ComponentTable` holds all the values ever inserted for a given component (provided they
/// are still alive, i.e. not GC'd).
///
/// Example of a component table holding instances:
/// ```text
/// ComponentTable {
///     name: rerun.instance_key
///     size: 2 buckets for a total of 128 B across 5 total rows
///     buckets: [
///         ComponentBucket {
///             size: 64 B across 3 rows
///             row range: from 0 to 0 (all inclusive)
///             archived: true
///             time ranges:
///                 - frame_nr: from #41 to #41 (all inclusive)
///             +------------------------------------------------------------------+
///             | rerun.instance_key                                               |
///             +------------------------------------------------------------------+
///             | []                                                               |
///             | [2382325256275464629, 9801782006807296871, 13644487945655724411] |
///             | [0, 1, 2]                                                        |
///             +------------------------------------------------------------------+
///         }
///         ComponentBucket {
///             size: 64 B across 2 rows
///             row range: from 3 to 4 (all inclusive)
///             archived: false
///             time ranges:
///                 - frame_nr: from #42 to #42 (all inclusive)
///                 - log_time: from 19:37:36.713798Z to 19:37:37.713798Z (all inclusive)
///             +-------------------------------------------------------------------+
///             | rerun.instance_key                                                |
///             +-------------------------------------------------------------------+
///             | [8907162807054976021, 14953141369327162382, 15742885776230395882] |
///             | [165204472818569687, 3210188998985913268, 13675065411448304501]   |
///             +-------------------------------------------------------------------+
///         }
///     ]
/// }
/// ```
///
/// Example of a component-table holding 2D positions:
/// ```text
/// ComponentTable {
///     name: rerun.point2d
///     size: 2 buckets for a total of 96 B across 4 total rows
///     buckets: [
///         ComponentBucket {
///             size: 64 B across 3 rows
///             row range: from 0 to 0 (all inclusive)
///             archived: true
///             time ranges:
///                 - log_time: from 19:37:35.713798Z to 19:37:35.713798Z (all inclusive)
///                 - frame_nr: from #41 to #42 (all inclusive)
///             +-------------------------------------------------------------------+
///             | rerun.point2d                                                     |
///             +-------------------------------------------------------------------+
///             | []                                                                |
///             | [{x: 2.4033058, y: 8.535466}, {x: 4.051945, y: 7.6194324}         |
///             | [{x: 1.4975989, y: 6.17476}, {x: 2.4128711, y: 1.853013}          |
///             +-------------------------------------------------------------------+
///         }
///         ComponentBucket {
///             size: 32 B across 1 rows
///             row range: from 3 to 3 (all inclusive)
///             archived: false
///             time ranges:
///                 - frame_nr: from #44 to #44 (all inclusive)
///             +-------------------------------------------------------------------+
///             | rerun.point2d                                                     |
///             +-------------------------------------------------------------------+
///             | [{x: 0.6296742, y: 6.7517242}, {x: 2.3393118, y: 8.770799}        |
///             +-------------------------------------------------------------------+
///         }
///     ]
/// }
/// ```
#[derive(Debug)]
pub struct ComponentTable {
    /// Name of the underlying component.
    pub(crate) name: ComponentName,

    /// Type of the underlying component.
    pub(crate) datatype: DataType,

    /// The actual buckets, where the component data is stored.
    ///
    /// Component buckets are append-only, they can never be written to in an out of order
    /// fashion.
    /// As such, a double-ended queue covers all our needs:
    /// - popping from the front for garbage collection
    /// - pushing to the back for insertions
    /// - binary search for queries
    pub(crate) buckets: VecDeque<ComponentBucket>,
}

/// A `ComponentBucket` holds a size-delimited (data size) chunk of a [`ComponentTable`].
#[derive(Debug)]
pub struct ComponentBucket {
    /// The component's name, for debugging purposes.
    pub(crate) name: ComponentName,

    /// The offset of this bucket in the global table.
    pub(crate) row_offset: u64,

    /// Has this bucket been archived yet?
    ///
    /// For every `ComponentTable`, there can only be one active bucket at a time (i.e. the bucket
    /// that is currently accepting write requests), all the others are archived.
    /// When the currently active bucket is full, it is archived in turn, and a new bucket is
    /// created to take its place.
    ///
    /// Archiving a bucket is a good opportunity to run some maintenance tasks on it, e.g.
    /// compaction (concatenating all chunks down to a single one).
    /// Currently, an archived bucket is guaranteed to have these properties:
    /// - the bucket is full (it has reached the maximum allowed length and/or size),
    /// - the bucket has been compacted,
    /// - the bucket is only used for reads.
    pub(crate) archived: bool,

    /// The time ranges (plural!) covered by this bucket.
    /// Buckets are never sorted over time, so these time ranges can grow arbitrarily large.
    ///
    /// These are only used for garbage collection.
    pub(crate) time_ranges: HashMap<Timeline, TimeRange>,

    /// All the data for this bucket: many rows of a single column.
    ///
    /// Each chunk is a list of arrays of structs, i.e. `ListArray<StructArray>`:
    /// - the list layer corresponds to the different rows,
    /// - the array layer corresponds to the different instances within a single row,
    /// - and finally the struct layer holds the components themselves.
    /// E.g.:
    /// ```text
    /// [
    ///   [{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}],
    ///   [{x: 7.158843, y: 0.68897724}, {x: 8.934421, y: 2.8420508}],
    /// ]
    /// ```
    ///
    /// During the active lifespan of the bucket, this can contain any number of chunks,
    /// depending on how the data was inserted (e.g. single insertions vs. batches).
    /// All of these chunks get compacted into one contiguous array when the bucket is archived,
    /// i.e. when the bucket is full and a new one is created.
    ///
    /// Note that, as of today, we do not actually support batched insertion nor do we support
    /// chunks of non-unit length (batches are inserted on a per-row basis internally).
    /// As a result, chunks always contain one and only one row's worth of data, at least until
    /// the bucket is archived and compacted.
    /// See also #589.
    pub(crate) chunks: Vec<Box<dyn Array>>,

    /// The total number of rows present in this bucket, across all chunks.
    pub(crate) total_rows: u64,

    /// The size of this bucket in bytes, across all chunks.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    pub(crate) total_size_bytes: u64,
}
