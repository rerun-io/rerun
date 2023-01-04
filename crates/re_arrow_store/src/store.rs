use std::collections::{BTreeMap, HashMap, VecDeque};
use std::num::NonZeroU64;
use std::sync::atomic::AtomicU64;

use anyhow::{anyhow, ensure};
use arrow2::array::{Array, UInt64Array};
use arrow2::datatypes::DataType;

use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_format::{arrow, format_bytes, format_number};
use re_log_types::{
    ComponentName, MsgId, ObjPath as EntityPath, ObjPathHash as EntityPathHash, TimeInt, TimePoint,
    TimeRange, Timeline,
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

/// An opaque type that directly refers to a row of data within the datastore, iff it is associated
/// with a component name.
///
/// See [`DataStore::latest_at`], [`DataStore::range`] & [`DataStore::get`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowIndex(pub(crate) NonZeroU64);
impl RowIndex {
    /// Panics if `v` is 0.
    pub(crate) fn from_u64(v: u64) -> Self {
        Self(v.try_into().unwrap())
    }

    pub(crate) fn as_u64(self) -> u64 {
        self.0.into()
    }
}
impl std::fmt::Display for RowIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexRowNr(pub(crate) u64);
impl std::fmt::Display for IndexRowNr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

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

    /// Used to cache auto-generated cluster components, i.e. `[0]`, `[0, 1]`, `[0, 1, 2]`, etc
    /// so that they can be properly deduplicated.
    pub(crate) cluster_comp_cache: IntMap<usize, RowIndex>,

    /// Maps `MsgId`s to some metadata (just timepoints at the moment).
    ///
    /// `BTreeMap` because of garbage collection.
    pub(crate) messages: BTreeMap<MsgId, TimePoint>,

    /// Maps an entity to its index, for a specific timeline.
    ///
    /// An index maps specific points in time to rows in component tables.
    pub(crate) indices: HashMap<(Timeline, EntityPathHash), IndexTable>,

    /// Maps a component name to its associated table, for all timelines and all entities.
    ///
    /// A component table holds all the values ever inserted for a given component.
    pub(crate) components: IntMap<ComponentName, ComponentTable>,

    /// Dedicated index tables for timeless data. Never garbage collected.
    pub(crate) timeless_indices: IntMap<EntityPathHash, PersistentIndexTable>,

    /// Dedicated component tables for timeless data. Never garbage collected.
    pub(crate) timeless_components: IntMap<ComponentName, PersistentComponentTable>,

    /// Monotically increasing ID for insertions.
    pub(crate) insert_id: u64,
    /// Monotically increasing ID for queries.
    pub(crate) query_id: AtomicU64,
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

    /// Returns the number of timeless index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its timeless index tables.
    pub fn total_timeless_index_rows(&self) -> u64 {
        self.timeless_indices
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its timeless index tables, in bytes.
    pub fn total_timeless_index_size_bytes(&self) -> u64 {
        self.timeless_indices
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of timeless component rows stored across this entire store, i.e. the
    /// sum of the number of rows across all of its timeless component tables.
    pub fn total_timeless_component_rows(&self) -> u64 {
        self.timeless_components
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless component data stored across this entire store, i.e. the
    /// sum of the size of the data stored across all of its timeless component tables, in bytes.
    pub fn total_timeless_component_size_bytes(&self) -> u64 {
        self.timeless_components
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its index tables.
    ///
    /// This doesn't account for timeless data!
    pub fn total_index_rows(&self) -> u64 {
        self.indices.values().map(|table| table.total_rows()).sum()
    }

    /// Returns the size of the index data stored across this entire store, i.e. the sum of
    /// the size of the data stored across all of its index tables, in bytes.
    ///
    /// This doesn't account for timeless data!
    pub fn total_index_size_bytes(&self) -> u64 {
        self.indices
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of component rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its component tables.
    ///
    /// This doesn't account for timeless data!
    pub fn total_component_rows(&self) -> u64 {
        self.components
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the component data stored across this entire store, i.e. the sum of
    /// the size of the data stored across all of its component tables, in bytes.
    ///
    /// This doesn't account for timeless data!
    pub fn total_component_size_bytes(&self) -> u64 {
        self.components
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Runs the sanity check suite for the entire datastore.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        // Row indices should be continuous across all index tables.
        // TODO(#449): update this one appropriately when GC lands.
        {
            let mut row_indices: IntMap<_, Vec<RowIndex>> = IntMap::default();
            for table in self.indices.values() {
                for bucket in table.buckets.values() {
                    for (comp, index) in &bucket.indices.read().indices {
                        let row_indices = row_indices.entry(*comp).or_default();
                        row_indices.extend(index.iter().copied().flatten());
                    }
                }
            }

            for (comp, mut row_indices) in row_indices {
                row_indices.sort();
                row_indices.dedup();
                for pair in row_indices.windows(2) {
                    let &[i1, i2] = pair else { unreachable!() };
                    ensure!(
                        i1.as_u64() + 1 == i2.as_u64(),
                        "found hole in index coverage for {comp:?}: \
                            in {row_indices:?}, {i1} -> {i2}"
                    );
                }
            }
        }

        // Row indices should be continuous across all timeless index tables.
        {
            let mut row_indices: IntMap<_, Vec<RowIndex>> = IntMap::default();
            for table in self.timeless_indices.values() {
                for (comp, index) in &table.indices {
                    let row_indices = row_indices.entry(*comp).or_default();
                    row_indices.extend(index.iter().copied().flatten());
                }
            }

            for (comp, mut row_indices) in row_indices {
                row_indices.sort();
                row_indices.dedup();
                for pair in row_indices.windows(2) {
                    let &[i1, i2] = pair else { unreachable!() };
                    ensure!(
                        i1.as_u64() + 1 == i2.as_u64(),
                        "found hole in timeless index coverage for {comp:?}: \
                            in {row_indices:?}, {i1} -> {i2}"
                    );
                }
            }
        }

        for table in self.timeless_indices.values() {
            table.sanity_check()?;
        }

        for table in self.timeless_components.values() {
            table.sanity_check()?;
        }

        for table in self.indices.values() {
            table.sanity_check()?;
        }

        for table in self.components.values() {
            table.sanity_check()?;
        }

        Ok(())
    }
}

impl std::fmt::Display for DataStore {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cluster_key,
            config,
            cluster_comp_cache: _,
            messages: _,
            indices,
            components,
            timeless_indices,
            timeless_components,
            insert_id: _,
            query_id: _,
        } = self;

        f.write_str("DataStore {\n")?;

        f.write_str(&indent::indent_all_by(
            4,
            format!("cluster_key: {cluster_key:?}\n"),
        ))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} timeless index tables, for a total of {} across {} total rows\n",
                    timeless_indices.len(),
                    format_bytes(self.total_timeless_index_size_bytes() as _),
                    format_number(self.total_timeless_index_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "timeless_indices: [\n"))?;
            for table in timeless_indices.values() {
                f.write_str(&indent::indent_all_by(8, "PersistentIndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }
        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} persistent component tables, for a total of {} across {} total rows\n",
                    timeless_components.len(),
                    format_bytes(self.total_timeless_component_size_bytes() as _),
                    format_number(self.total_timeless_component_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "timeless_components: [\n"))?;
            for table in timeless_components.values() {
                f.write_str(&indent::indent_all_by(8, "PersistentComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} index tables, for a total of {} across {} total rows\n",
                    indices.len(),
                    format_bytes(self.total_index_size_bytes() as _),
                    format_number(self.total_index_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "indices: [\n"))?;
            for table in indices.values() {
                f.write_str(&indent::indent_all_by(8, "IndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }
        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} component tables, for a total of {} across {} total rows\n",
                    components.len(),
                    format_bytes(self.total_component_size_bytes() as _),
                    format_number(self.total_component_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
            for table in components.values() {
                f.write_str(&indent::indent_all_by(8, "ComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        f.write_str("}")?;

        Ok(())
    }
}

// --- Persistent Indices ---

/// A `PersistentIndexTable` maps specific entries to rows in persistent component tables.
///
/// Example of a time-based index table (`MAX_ROWS_PER_BUCKET=2`):
/// TODO
///
/// Example of a sequence-based index table (`MAX_ROWS_PER_BUCKET=2`):
/// TODO
#[derive(Debug)]
pub struct PersistentIndexTable {
    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,

    /// Carrying the cluster key around to help with assertions and sanity checks all over the
    /// place.
    pub(crate) cluster_key: ComponentName,

    /// The number of rows in the table: all indices should always be exactly of that length.
    pub(crate) nb_rows: u64,

    /// All component indices for this bucket.
    ///
    /// One index per component: new components (and as such, new indices) can be added at any
    /// time!
    /// When that happens, they will be retro-filled with nulls until they are [`Self::nb_rows`]
    /// long.
    pub(crate) indices: IntMap<ComponentName, SecondaryIndex>,
}

impl std::fmt::Display for PersistentIndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            ent_path,
            cluster_key: _,
            nb_rows: _,
            indices: _,
        } = self;

        f.write_fmt(format_args!("entity: {}\n", ent_path))?;

        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        let (col_names, cols) = self.named_indices();

        let names = col_names.into_iter().map(|name| name.to_string());
        let values = cols.into_iter().map(|c| c.boxed());
        let table = arrow::format_table(values, names);

        f.write_fmt(format_args!("data:\n{table}\n"))?;

        Ok(())
    }
}

impl PersistentIndexTable {
    // pub fn entity_path(&self) -> &EntityPath {
    //     &self.ent_path
    // }

    /// Returns the number of rows stored across this table.
    pub fn total_rows(&self) -> u64 {
        self.nb_rows
    }

    /// Returns the size of the data stored across this table, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.indices
            .values()
            .map(|index| std::mem::size_of_val(index.as_slice()) as u64)
            .sum::<u64>()
    }

    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        let Self {
            ent_path: _,
            cluster_key,
            nb_rows,
            indices,
        } = self;

        // All indices should be `Self::nb_rows` long.
        {
            for (comp, index) in indices {
                let secondary_len = index.len() as u64;
                ensure!(
                    *nb_rows == secondary_len,
                    "found rogue secondary index for {comp:?}: \
                        expected {nb_rows} rows, got {secondary_len} instead",
                );
            }
        }

        // The cluster index must be fully dense.
        {
            let cluster_idx = indices
                .get(cluster_key)
                .ok_or_else(|| anyhow!("no index found for cluster key: {cluster_key:?}"))?;
            ensure!(
                cluster_idx.iter().all(|row| row.is_some()),
                "the cluster index ({cluster_key:?}) must be fully dense: \
                    got {cluster_idx:?}",
            );
        }

        Ok(())
    }

    pub fn named_indices(&self) -> (Vec<ComponentName>, Vec<UInt64Array>) {
        self.indices
            .iter()
            .map(|(name, index)| {
                (
                    name,
                    UInt64Array::from(
                        index
                            .iter()
                            .map(|row_idx| row_idx.map(|row_idx| row_idx.as_u64()))
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .unzip()
    }
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
///             +-------------------------------+--------------+---------------+----------------+
///             | log_time                      | rerun.rect2d | rerun.point2d | rerun.instance |
///             +-------------------------------+--------------+---------------+----------------+
///             | 2022-12-20 19:37:35.713798552 |              | 2             | 2              |
///             | 2022-12-20 19:37:35.713798552 | 4            |               | 2              |
///             +-------------------------------+--------------+---------------+----------------+
///
///         }
///         IndexBucket {
///             index time bound: >= 19:37:36.713798Z
///             size: 64 B across 2 rows
///                 - log_time: from 19:37:36.713798Z to 19:37:36.713798Z (all inclusive)
///             data (sorted=true):
///             +-------------------------------+--------------+----------------+---------------+
///             | log_time                      | rerun.rect2d | rerun.instance | rerun.point2d |
///             +-------------------------------+--------------+----------------+---------------+
///             | 2022-12-20 19:37:36.713798552 | 1            | 2              |               |
///             | 2022-12-20 19:37:36.713798552 |              | 4              |               |
///             +-------------------------------+--------------+----------------+---------------+
///
///         }
///         IndexBucket {
///             index time bound: >= 19:37:37.713798Z
///             size: 24 B across 1 rows
///                 - log_time: from 19:37:37.713798Z to 19:37:37.713798Z (all inclusive)
///             data (sorted=true):
///             +-------------------------------+--------------+----------------+
///             | log_time                      | rerun.rect2d | rerun.instance |
///             +-------------------------------+--------------+----------------+
///             | 2022-12-20 19:37:37.713798552 | 2            | 3              |
///             +-------------------------------+--------------+----------------+
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
///             +----------+---------------+--------------+----------------+
///             | frame_nr | rerun.point2d | rerun.rect2d | rerun.instance |
///             +----------+---------------+--------------+----------------+
///             | 41       |               |              | 1              |
///             | 41       | 1             |              | 2              |
///             | 41       |               | 3            | 2              |
///             +----------+---------------+--------------+----------------+
///
///         }
///         IndexBucket {
///             index time bound: >= #42
///             size: 96 B across 3 rows
///                 - frame_nr: from #42 to #42 (all inclusive)
///             data (sorted=true):
///             +----------+--------------+----------------+---------------+
///             | frame_nr | rerun.rect2d | rerun.instance | rerun.point2d |
///             +----------+--------------+----------------+---------------+
///             | 42       | 1            | 2              |               |
///             | 42       |              | 4              |               |
///             | 42       |              | 2              | 2             |
///             +----------+--------------+----------------+---------------+
///
///         }
///         IndexBucket {
///             index time bound: >= #43
///             size: 64 B across 2 rows
///                 - frame_nr: from #43 to #44 (all inclusive)
///             data (sorted=true):
///             +----------+--------------+---------------+----------------+
///             | frame_nr | rerun.rect2d | rerun.point2d | rerun.instance |
///             +----------+--------------+---------------+----------------+
///             | 43       | 4            |               | 2              |
///             | 44       |              | 3             | 2              |
///             +----------+--------------+---------------+----------------+
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
}

impl std::fmt::Display for IndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
            cluster_key: _,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {}\n", ent_path))?;

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;
        f.write_str("buckets: [\n")?;
        for (time, bucket) in buckets.iter() {
            f.write_str(&indent::indent_all_by(4, "IndexBucket {\n"))?;
            f.write_str(&indent::indent_all_by(
                8,
                format!("index time bound: >= {}\n", timeline.typ().format(*time),),
            ))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string()))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl IndexTable {
    pub fn entity_path(&self) -> &EntityPath {
        &self.ent_path
    }

    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    pub fn total_rows(&self) -> u64 {
        self.buckets
            .values()
            .map(|bucket| bucket.total_rows())
            .sum()
    }

    /// Returns the size of data stored across this entire table, i.e. the sum of the size of
    /// the data stored across all of its buckets, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.buckets
            .values()
            .map(|bucket| bucket.total_size_bytes())
            .sum()
    }

    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        // No two buckets should ever overlap time-range-wise.
        {
            let time_ranges = self
                .buckets
                .values()
                .map(|bucket| bucket.indices.read().time_range)
                .collect::<Vec<_>>();
            for time_ranges in time_ranges.windows(2) {
                let &[t1, t2] = time_ranges else { unreachable!() };
                ensure!(
                    t1.max.as_i64() < t2.min.as_i64(),
                    "found overlapping index buckets: {} ({}) <-> {} ({})",
                    self.timeline.typ().format(t1.max),
                    t1.max.as_i64(),
                    self.timeline.typ().format(t2.min),
                    t2.min.as_i64(),
                );
            }
        }

        // Run individual bucket sanity check suites too.
        for bucket in self.buckets.values() {
            bucket.sanity_check()?;
        }

        Ok(())
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

impl std::fmt::Display for IndexBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        f.write_fmt(format_args!("{}\n", self.formatted_time_range()))?;

        let (timeline_name, times) = self.times();
        let (col_names, cols) = self.named_indices();

        let names = std::iter::once(timeline_name)
            .chain(col_names.into_iter().map(|name| name.to_string()));
        let values = std::iter::once(times.boxed()).chain(cols.into_iter().map(|c| c.boxed()));
        let table = arrow::format_table(values, names);

        let is_sorted = self.is_sorted();
        f.write_fmt(format_args!("data (sorted={is_sorted}):\n{table}\n"))?;

        Ok(())
    }
}

impl IndexBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.indices.read().times.len() as u64
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        std::mem::size_of_val(times.as_slice()) as u64
            + indices
                .values()
                .map(|index| std::mem::size_of_val(index.as_slice()) as u64)
                .sum::<u64>()
    }

    /// Returns a formatted string of the time range in the bucket
    pub fn formatted_time_range(&self) -> String {
        let time_range = &self.indices.read().time_range;
        if time_range.min.as_i64() != i64::MAX && time_range.max.as_i64() != i64::MIN {
            self.timeline.format_time_range(time_range)
        } else {
            "time range: N/A\n".to_owned()
        }
    }

    /// Runs the sanity check suite for the entire bucket.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        // All indices should contain the exact same number of rows as the time index.
        {
            let primary_len = times.len();
            for (comp, index) in indices {
                let secondary_len = index.len();
                ensure!(
                    primary_len == secondary_len,
                    "found rogue secondary index for {comp:?}: \
                        expected {primary_len} rows, got {secondary_len} instead",
                );
            }
        }

        // The cluster index must be fully dense.
        {
            let cluster_key = self.cluster_key;
            let cluster_idx = indices
                .get(&cluster_key)
                .ok_or_else(|| anyhow!("no index found for cluster key: {cluster_key:?}"))?;
            ensure!(
                cluster_idx.iter().all(|row| row.is_some()),
                "the cluster index ({cluster_key:?}) must be fully dense: \
                    got {cluster_idx:?}",
            );
        }

        Ok(())
    }
}

// --- Persistent Components ---

/// A `PersistentComponentTable` holds all the timeless values ever inserted for a given component.
///
/// Example of a component table holding instances:
/// TODO
///
/// Example of a component-table holding 2D positions:
/// TODO
#[derive(Debug)]
pub struct PersistentComponentTable {
    /// Name of the underlying component, for debugging purposes.
    pub(crate) name: ComponentName,
    /// The entity this table is related to, for debugging purposes.
    pub(crate) ent_path: EntityPath,
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
    /// All of these chunks get merged into one contiguous array when the table itself is
    /// compacted, which happens... TODO: when???????????
    ///
    /// Note that, as of today, we do not actually support batched insertion nor do we support
    /// chunks of non-unit length (batches are inserted on a per-row basis internally).
    /// As a result, chunks always contain one and only one row's worth of data, at least until
    /// the bucket is compacted one or more times.
    /// See also #589.
    //
    // TODO: compact those too
    pub(crate) chunks: Vec<Box<dyn Array>>,

    /// The total number of rows present in this bucket, across all chunks.
    pub(crate) total_rows: u64,
    /// The size of this bucket in bytes, across all chunks.
    ///
    /// Accurately computing the size of arrow arrays is surprisingly costly, which is why we
    /// cache this.
    pub(crate) total_size_bytes: u64,
}

impl std::fmt::Display for PersistentComponentTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            ent_path,
            datatype,
            chunks,
            total_rows,
            total_size_bytes,
        } = self;

        f.write_fmt(format_args!("name: {}\n", name))?;
        f.write_fmt(format_args!("entity: {}\n", ent_path))?;
        if matches!(
            std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS").as_deref(),
            Ok("1")
        ) {
            f.write_fmt(format_args!("datatype: {:#?}\n", datatype))?;
        }

        f.write_fmt(format_args!(
            "size: {} across {} total rows\n",
            format_bytes(*total_size_bytes as _),
            format_number(*total_rows as _),
        ))?;

        let data = {
            use arrow2::compute::concatenate::concatenate;
            let chunks = chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            concatenate(&chunks).unwrap()
        };

        let table = arrow::format_table([data], [self.name.as_str()]);
        f.write_fmt(format_args!("{table}\n"))?;

        Ok(())
    }
}

impl PersistentComponentTable {
    /// Returns the number of rows stored across this table.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Returns the size of the data stored across this table, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }

    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        // TODO: shouldn't components always be dense?
        Ok(())
    }
}

// --- Components ---

/// A `ComponentTable` holds all the values ever inserted for a given component (provided they
/// are still alive, i.e. not GC'd).
///
/// Example of a component table holding instances:
/// ```text
/// ComponentTable {
///     name: rerun.instance
///     size: 2 buckets for a total of 128 B across 5 total rows
///     buckets: [
///         ComponentBucket {
///             size: 64 B across 3 rows
///             row range: from 0 to 0 (all inclusive)
///             archived: true
///             time ranges:
///                 - frame_nr: from #41 to #41 (all inclusive)
///             +------------------------------------------------------------------+
///             | rerun.instance                                                   |
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
///             | rerun.instance                                                    |
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
    /// - poping from the front for garbage collection
    /// - pushing to the back for insertions
    /// - binary search for queries
    pub(crate) buckets: VecDeque<ComponentBucket>,
}

impl std::fmt::Display for ComponentTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            buckets,
        } = self;

        f.write_fmt(format_args!("name: {}\n", name))?;
        if matches!(
            std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS").as_deref(),
            Ok("1")
        ) {
            f.write_fmt(format_args!("datatype: {:#?}\n", datatype))?;
        }

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;
        f.write_str("buckets: [\n")?;
        for bucket in buckets {
            f.write_str(&indent::indent_all_by(4, "ComponentBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string()))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl ComponentTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    pub fn total_rows(&self) -> u64 {
        self.buckets.iter().map(|bucket| bucket.total_rows()).sum()
    }

    /// Returns the size of data stored across this entire table, i.e. the sum of the size of
    /// the data stored across all of its buckets, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.buckets
            .iter()
            .map(|bucket| bucket.total_size_bytes())
            .sum()
    }

    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> anyhow::Result<()> {
        // No two buckets should ever overlap row-range-wise.
        {
            let row_ranges = self
                .buckets
                .iter()
                .map(|bucket| bucket.row_offset..bucket.row_offset + bucket.total_rows())
                .collect::<Vec<_>>();
            for row_ranges in row_ranges.windows(2) {
                let &[r1, r2] = &row_ranges else { unreachable!() };
                ensure!(
                    !r1.contains(&r2.start),
                    "found overlapping component buckets: {r1:?} <-> {r2:?}"
                );
            }
        }

        // TODO: shouldn't components always be dense?
        Ok(())
    }
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

impl std::fmt::Display for ComponentBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        f.write_fmt(format_args!(
            "row range: from {} to {} (all inclusive)\n",
            self.row_offset,
            // Component buckets can never be empty at the moment:
            // - the first bucket is always initialized with a single empty row
            // - all buckets that follow are lazily instantiated when data get inserted
            //
            // TODO(#439): is that still true with deletion?
            // TODO(#589): support for non-unit-length chunks
            self.row_offset
                + self
                    .chunks
                    .len()
                    .checked_sub(1)
                    .expect("buckets are never empty") as u64,
        ))?;

        f.write_fmt(format_args!("archived: {}\n", self.archived))?;
        f.write_str("time ranges:\n")?;
        for (timeline, time_range) in &self.time_ranges {
            f.write_fmt(format_args!(
                "{}\n",
                &timeline.format_time_range(time_range)
            ))?;
        }

        let data = {
            use arrow2::compute::concatenate::concatenate;
            let chunks = self.chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            concatenate(&chunks).unwrap()
        };

        let table = arrow::format_table([data], [self.name.as_str()]);
        f.write_fmt(format_args!("{table}\n"))?;

        Ok(())
    }
}

impl ComponentBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }
}

// This test exists because the documentation and online discussions revolving around
// arrow2's `estimated_bytes_size()` function indicate that there's a lot of limitations and
// edge cases to be aware of.
//
// Also, it's just plain hard to be sure that the answer you get is the answer you're looking
// for with these kinds of tools. When in doubt.. test everything we're going to need from it.
//
// In many ways, this is a specification of what we mean when we ask "what's the size of this
// Arrow array?".
#[test]
#[allow(clippy::from_iter_instead_of_collect)]
fn test_arrow_estimated_size_bytes() {
    use arrow2::{
        array::{Float64Array, ListArray, StructArray, UInt64Array, Utf8Array},
        compute::aggregate::estimated_bytes_size,
        datatypes::{DataType, Field},
        offset::Offsets,
    };

    // simple primitive array
    {
        let data = vec![42u64; 100];
        let array = UInt64Array::from_vec(data.clone()).boxed();
        assert_eq!(
            std::mem::size_of_val(data.as_slice()),
            estimated_bytes_size(&*array)
        );
    }

    // utf8 strings array
    {
        let data = vec![Some("some very, very, very long string indeed"); 100];
        let array = Utf8Array::<i32>::from(data.clone()).to_boxed();

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.unwrap().as_bytes()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(5600, raw_size_bytes);
        assert_eq!(4404, arrow_size_bytes); // smaller because validity bitmaps instead of opts
    }

    // simple primitive list array
    {
        let data = std::iter::repeat(vec![42u64; 100])
            .take(50)
            .collect::<Vec<_>>();
        let array = {
            let array_flattened =
                UInt64Array::from_vec(data.clone().into_iter().flatten().collect()).boxed();

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(DataType::UInt64),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array_flattened,
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(41200, raw_size_bytes);
        assert_eq!(40200, arrow_size_bytes); // smaller because smaller inner headers
    }

    // compound type array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }
        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = vec![Point::default(); 100];
        let array = {
            let x = Float64Array::from_vec(data.iter().map(|p| p.x).collect()).boxed();
            let y = Float64Array::from_vec(data.iter().map(|p| p.y).collect()).boxed();
            let fields = vec![
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
            ];
            StructArray::new(DataType::Struct(fields), vec![x, y], None).boxed()
        };

        let raw_size_bytes = std::mem::size_of_val(data.as_slice());
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(1600, raw_size_bytes);
        assert_eq!(1600, arrow_size_bytes);
    }

    // compound type list array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }
        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = std::iter::repeat(vec![Point::default(); 100])
            .take(50)
            .collect::<Vec<_>>();
        let array: Box<dyn Array> = {
            let array = {
                let x =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.x).collect()).boxed();
                let y =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.y).collect()).boxed();
                let fields = vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                ];
                StructArray::new(DataType::Struct(fields), vec![x, y], None)
            };

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(array.data_type().clone()),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array.boxed(),
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(81200, raw_size_bytes);
        assert_eq!(80200, arrow_size_bytes); // smaller because smaller inner headers
    }
}
