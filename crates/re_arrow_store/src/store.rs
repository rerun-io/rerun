use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::ensure;
use arrow2::array::{Array, Int64Vec, MutableArray, UInt64Vec};
use arrow2::bitmap::MutableBitmap;
use arrow2::datatypes::DataType;

use parking_lot::RwLock;
use re_format::{format_bytes, format_number};
use re_log_types::{
    ComponentName, ObjPath as EntityPath, ObjPathHash as EntityPathHash, TimeInt, TimeRange,
    Timeline,
};

pub type RowIndex = u64;

// --- Data store ---

#[derive(Debug, Clone)]
pub struct DataStoreConfig {
    /// The maximum size of a component bucket before triggering a split.
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
    };
}

// ---

/// A complete data store: covers all timelines, all entities, everything.
///
/// `DataStore` provides a very thorough `Display` implementation that makes it manageable to
/// know what's going on internally.
/// For even more information, you can set `RERUN_DATA_STORE_DISPLAY_SCHEMAS=1` in your
/// environment, which will result in additional schema information being printed out.
#[derive(Default)]
pub struct DataStore {
    /// The configuration of the data store (e.g. bucket sizes).
    pub(crate) config: DataStoreConfig,

    /// Maps an entity to its index, for a specific timeline.
    ///
    /// An index maps specific points in time to rows in component tables.
    pub(crate) indices: HashMap<(Timeline, EntityPathHash), IndexTable>,

    /// Maps a component name to its associated table, for all timelines and all entities.
    ///
    /// A component table holds all the values ever inserted for a given component.
    pub(crate) components: HashMap<ComponentName, ComponentTable>,

    /// Monotically increasing ID for insertions.
    pub(crate) insert_id: u64,
    /// Monotically increasing ID for queries.
    pub(crate) query_id: AtomicU64,
}

impl DataStore {
    pub fn new(config: DataStoreConfig) -> Self {
        Self {
            config,
            indices: HashMap::default(),
            components: HashMap::default(),
            insert_id: 0,
            query_id: AtomicU64::new(0),
        }
    }

    /// Returns the number of index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its index tables.
    pub fn total_index_rows(&self) -> u64 {
        self.indices.values().map(|table| table.total_rows()).sum()
    }

    /// Returns the size of the index data stored across this entire store, i.e. the sum of
    /// the size of the data stored across all of its index tables, in bytes.
    pub fn total_index_size_bytes(&self) -> u64 {
        self.indices
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of component rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its component tables.
    pub fn total_component_rows(&self) -> u64 {
        self.components
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the component data stored across this entire store, i.e. the sum of
    /// the size of the data stored across all of its component tables, in bytes.
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
            let mut row_indices: HashMap<_, Vec<RowIndex>> = HashMap::new();
            for table in self.indices.values() {
                for bucket in table.buckets.values() {
                    for (comp, index) in &bucket.indices.read().indices {
                        let row_indices = row_indices.entry(comp.clone()).or_default();
                        row_indices.extend(index.values());
                    }
                }
            }

            for (comp, mut row_indices) in row_indices {
                row_indices.sort();
                row_indices.dedup();
                for pair in row_indices.windows(2) {
                    let &[i1, i2] = pair else { unreachable!() };
                    ensure!(
                        i1 + 1 == i2,
                        "found hole in index coverage for {comp:?}: \
                            in {row_indices:?}, {i1} -> {i2}"
                    );
                }
            }
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
            config,
            indices,
            components,
            insert_id: _,
            query_id: _,
        } = self;

        f.write_str("DataStore {\n")?;

        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} index tables, for a total of {} across {} total rows\n",
                    self.indices.len(),
                    format_bytes(self.total_index_size_bytes() as _),
                    format_number(self.total_index_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "indices: [\n"))?;
            for index in indices.values() {
                f.write_str(&indent::indent_all_by(8, "IndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, index.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} component tables, for a total of {} across {} total rows\n",
                    self.components.len(),
                    format_bytes(self.total_component_size_bytes() as _),
                    format_number(self.total_component_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
            for comp in components.values() {
                f.write_str(&indent::indent_all_by(8, "ComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, comp.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        f.write_str("}")?;

        Ok(())
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
///     size: 3 buckets for a total of 160 B across 5 total rows
///     buckets: [
///         IndexBucket {
///             size: 67 B across 2 rows
///             time range: from -∞ to 18:24:23.712311Z (all inclusive)
///             data (sorted=true): shape: (2, 4)
///             ┌──────────────────┬───────┬───────────┬───────────┐
///             │ time             ┆ rects ┆ instances ┆ positions │
///             │ ---              ┆ ---   ┆ ---       ┆ ---       │
///             │ str              ┆ u64   ┆ u64       ┆ u64       │
///             ╞══════════════════╪═══════╪═══════════╪═══════════╡
///             │ 18:24:23.712311Z ┆ null  ┆ null      ┆ 2         │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ 18:24:23.712311Z ┆ 4     ┆ null      ┆ null      │
///             └──────────────────┴───────┴───────────┴───────────┘
///         }
///         IndexBucket {
///             size: 67 B across 2 rows
///             time range: from 18:24:24.712311Z to 18:24:24.712311Z (all inclusive)
///             data (sorted=true): shape: (2, 4)
///             ┌──────────────────┬───────┬───────────┬───────────┐
///             │ time             ┆ rects ┆ instances ┆ positions │
///             │ ---              ┆ ---   ┆ ---       ┆ ---       │
///             │ str              ┆ u64   ┆ u64       ┆ u64       │
///             ╞══════════════════╪═══════╪═══════════╪═══════════╡
///             │ 18:24:24.712311Z ┆ 1     ┆ null      ┆ null      │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ 18:24:24.712311Z ┆ null  ┆ 3         ┆ null      │
///             └──────────────────┴───────┴───────────┴───────────┘
///         }
///         IndexBucket {
///             size: 26 B across 1 rows
///             time range: from 18:24:25.712311Z to +∞ (all inclusive)
///             data (sorted=true): shape: (1, 3)
///             ┌──────────────────┬───────┬───────────┐
///             │ time             ┆ rects ┆ instances │
///             │ ---              ┆ ---   ┆ ---       │
///             │ str              ┆ u64   ┆ u64       │
///             ╞══════════════════╪═══════╪═══════════╡
///             │ 18:24:25.712311Z ┆ 2     ┆ 2         │
///             └──────────────────┴───────┴───────────┘
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
///     size: 3 buckets for a total of 265 B across 8 total rows
///     buckets: [
///         IndexBucket {
///             size: 99 B across 3 rows
///             time range: from -∞ to #41 (all inclusive)
///             data (sorted=true): shape: (3, 4)
///             ┌──────┬───────────┬───────────┬───────┐
///             │ time ┆ instances ┆ positions ┆ rects │
///             │ ---  ┆ ---       ┆ ---       ┆ ---   │
///             │ str  ┆ u64       ┆ u64       ┆ u64   │
///             ╞══════╪═══════════╪═══════════╪═══════╡
///             │ #41  ┆ 1         ┆ null      ┆ null  │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #41  ┆ null      ┆ 1         ┆ null  │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #41  ┆ null      ┆ null      ┆ 3     │
///             └──────┴───────────┴───────────┴───────┘
///         }
///         IndexBucket {
///             size: 99 B across 3 rows
///             time range: from #42 to #42 (all inclusive)
///             data (sorted=true): shape: (3, 4)
///             ┌──────┬───────────┬───────┬───────────┐
///             │ time ┆ instances ┆ rects ┆ positions │
///             │ ---  ┆ ---       ┆ ---   ┆ ---       │
///             │ str  ┆ u64       ┆ u64   ┆ u64       │
///             ╞══════╪═══════════╪═══════╪═══════════╡
///             │ #42  ┆ null      ┆ 1     ┆ null      │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ #42  ┆ 3         ┆ null  ┆ null      │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
///             │ #42  ┆ null      ┆ null  ┆ 2         │
///             └──────┴───────────┴───────┴───────────┘
///         }
///         IndexBucket {
///             size: 67 B across 2 rows
///             time range: from #43 to +∞ (all inclusive)
///             data (sorted=true): shape: (2, 4)
///             ┌──────┬───────────┬───────────┬───────┐
///             │ time ┆ positions ┆ instances ┆ rects │
///             │ ---  ┆ ---       ┆ ---       ┆ ---   │
///             │ str  ┆ u64       ┆ u64       ┆ u64   │
///             ╞══════╪═══════════╪═══════════╪═══════╡
///             │ #43  ┆ null      ┆ null      ┆ 4     │
///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
///             │ #44  ┆ 3         ┆ null      ┆ null  │
///             └──────┴───────────┴───────────┴───────┘
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
    pub(crate) buckets: BTreeMap<TimeInt, IndexBucket>,
}

impl std::fmt::Display for IndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
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
        for bucket in buckets.values() {
            f.write_str(&indent::indent_all_by(4, "IndexBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl IndexTable {
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
}

/// Just the indices, to simplify interior mutability.
#[derive(Debug)]
pub struct IndexBucketIndices {
    /// Whether the indices (all of them!) are currently sorted.
    ///
    /// Querying an `IndexBucket` will always trigger a sort if the indices aren't already sorted.
    pub(crate) is_sorted: bool,

    /// The time range covered by the primary time index.
    pub(crate) time_range: TimeRange,

    // The primary time index, which is guaranteed to be dense, and "drives" all other indices.
    //
    // All secondary indices are guaranteed to follow the same sort order and be the same length.
    pub(crate) times: Int64Vec,

    /// All secondary indices for this bucket (i.e. everything but time).
    ///
    /// One index per component: new components (and as such, new indices) can be added at any
    /// time!
    /// When that happens, they will be retro-filled with nulls so that they share the same
    /// length as the primary index.
    pub(crate) indices: HashMap<ComponentName, UInt64Vec>,
}

impl Default for IndexBucketIndices {
    fn default() -> Self {
        Self {
            is_sorted: true,
            time_range: TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            times: Int64Vec::default(),
            indices: Default::default(),
        }
    }
}

impl std::fmt::Display for IndexBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { timeline, indices } = self;

        let IndexBucketIndices {
            is_sorted,
            time_range,
            times,
            indices,
        } = &*indices.read();

        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        f.write_fmt(format_args!(
            "time range: from {} to {} (all inclusive)\n",
            timeline.typ().format(time_range.min),
            timeline.typ().format(time_range.max),
        ))?;

        #[cfg(not(target_arch = "wasm32"))]
        {
            use arrow2::array::MutableArray as _;
            use polars::prelude::{DataFrame, NamedFrom as _, Series};

            let typ = timeline.typ();
            let times = Series::new(
                "time",
                times
                    .values()
                    .iter()
                    .map(|time| typ.format(TimeInt::from(*time)))
                    .collect::<Vec<_>>(),
            );

            let series = std::iter::once(times)
                .chain(indices.iter().map(|(name, index)| {
                    let index = index
                        .values()
                        .iter()
                        .enumerate()
                        .map(|(i, v)| index.is_valid(i).then_some(*v))
                        .collect::<Vec<_>>();
                    Series::new(name.as_str(), index)
                }))
                .collect::<Vec<_>>();
            // TODO(cmc): sort component columns (not time!)
            let df = DataFrame::new(series).unwrap();
            f.write_fmt(format_args!("data (sorted={is_sorted}): {df:?}"))?;
        }

        #[cfg(target_arch = "wasm32")]
        {
            _ = time_range;
            _ = is_sorted;
            _ = times;
            _ = indices;
        }

        Ok(())
    }
}

impl IndexBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.indices.read().times.len() as u64
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    // NOTE: for mutable, non-erased arrays, it's actually easier to compute ourselves.
    pub fn total_size_bytes(&self) -> u64 {
        fn size_of_validity(bitmap: Option<&MutableBitmap>) -> u64 {
            bitmap.map_or(0, |bitmap| std::mem::size_of_val(bitmap.as_slice())) as _
        }
        fn size_of_values<T>(values: &Vec<T>) -> u64 {
            std::mem::size_of_val(values.as_slice()) as _
        }

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        size_of_validity(times.validity())
            + size_of_values(times.values())
            + indices
                .values()
                .map(|index| size_of_validity(index.validity()) + size_of_values(index.values()))
                .sum::<u64>()
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

        Ok(())
    }
}

// --- Components ---

/// A `ComponentTable` holds all the values ever inserted for a given component (provided they
/// are still alive, i.e. not GC'd).
///
/// Example of a component table holding instance IDs:
/// ```text
/// ComponentTable {
///     name: instances
///     buckets: [
///         ComponentBucket {
///             row offset: 0
///             time ranges:
///                 - frame_nr: from #41 (inclusive) to #43 (exlusive)
///                 - log_time: from 10:24:21.735485Z (inclusive) to 10:24:21.755485Z (exlusive)
///             data: shape: (3, 1)
///             ┌─────────────────────────────────────┐
///             │ instances                           │
///             │ ---                                 │
///             │ list[u32]                           │
///             ╞═════════════════════════════════════╡
///             │ []                                  │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [478150623, 125728625, 4153899129]  │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [1827991721, 3089121314, 427290248] │
///             └─────────────────────────────────────┘
///
///         }
///     ]
/// }
/// ```
///
/// Example of a component-table holding 2D positions:
///
/// ```text
/// ComponentTable {
///     name: positions
///     buckets: [
///         ComponentBucket {
///             row offset: 0
///             time ranges:
///                 - log_time: from 10:24:21.725485Z (inclusive) to 10:24:21.725485Z (exlusive)
///                 - frame_nr: from #42 (inclusive) to #43 (exlusive)
///             data: shape: (2, 1)
///             ┌────────────────────────────────────────────────────────────────┐
///             │ positions                                                      │
///             │ ---                                                            │
///             │ list[struct[2]]                                                │
///             ╞════════════════════════════════════════════════════════════════╡
///             │ []                                                             │
///             ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
///             │ [{6.172664,8.383976}, {2.059066,8.037471}, {0.42883,1.250902}] │
///             └────────────────────────────────────────────────────────────────┘
///
///         }
///     ]
/// }
/// ```
#[derive(Debug)]
pub struct ComponentTable {
    /// Name of the underlying component.
    pub(crate) name: Arc<ComponentName>,
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
            f.write_str(&indent::indent_all_by(8, bucket.to_string() + "\n"))?;
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

        Ok(())
    }
}

/// A `ComponentBucket` holds a size-delimited (data size) chunk of a [`ComponentTable`].
#[derive(Debug)]
pub struct ComponentBucket {
    /// The component's name, for debugging purposes.
    pub(crate) name: Arc<String>,
    /// The offset of this bucket in the global table.
    pub(crate) row_offset: RowIndex,

    /// The time ranges (plural!) covered by this bucket.
    /// Buckets are never sorted over time, so these time ranges can grow arbitrarily large.
    ///
    /// These are only used for garbage collection.
    pub(crate) time_ranges: HashMap<Timeline, TimeRange>,

    /// All the data for this bucket. This is a single column!
    pub(crate) data: Box<dyn Array>,
}

impl std::fmt::Display for ComponentBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            time_ranges,
            row_offset,
            data,
        } = self;

        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        f.write_fmt(format_args!(
            "row range: from {} to {} (all inclusive)\n",
            row_offset,
            // Component buckets can never be empty at the moment:
            // - the first bucket is always initialized with a single empty row
            // - all buckets that follow are lazily instantiated when data get inserted
            //
            // TODO(#439): is that still true with deletion?
            row_offset + data.len().checked_sub(1).expect("buckets are never empty") as u64,
        ))?;

        f.write_str("time ranges:\n")?;
        for (timeline, time_range) in time_ranges {
            f.write_fmt(format_args!(
                "    - {}: from {} to {} (all inclusive)\n",
                timeline.name(),
                timeline.typ().format(time_range.min),
                timeline.typ().format(time_range.max),
            ))?;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            use polars::prelude::{DataFrame, Series};
            // TODO(cmc): I'm sure there's no need to clone here
            let series = Series::try_from((name.as_str(), data.clone())).unwrap();
            let df = DataFrame::new(vec![series]).unwrap();
            f.write_fmt(format_args!("data: {df:?}"))?;
        }

        #[cfg(target_arch = "wasm32")]
        {
            _ = name;
            _ = time_ranges;
            _ = row_offset;
            _ = data;
        }

        Ok(())
    }
}

impl ComponentBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.data.len() as u64
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        arrow2::compute::aggregate::estimated_bytes_size(&*self.data) as u64
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
        buffer::Buffer,
        compute::aggregate::estimated_bytes_size,
        datatypes::{DataType, Field},
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

            let mut i = 0i32;
            let indices = std::iter::from_fn(move || {
                let ret = i;
                i += 50;
                Some(ret)
            });

            ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(DataType::UInt64),
                Buffer::from_iter(indices.take(50)),
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

            let mut i = 0i32;
            let indices = std::iter::from_fn(move || {
                let ret = i;
                i += 50;
                Some(ret)
            });

            ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(array.data_type().clone()),
                Buffer::from_iter(indices.take(50)),
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
