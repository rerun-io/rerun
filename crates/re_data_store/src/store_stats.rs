use re_log_types::{EntityPathHash, TimePoint, TimeRange};
use re_types_core::SizeBytes;

use crate::{store::IndexedBucketInner, DataStore, IndexedBucket, IndexedTable, MetadataRegistry};

// ---

// NOTE: Not implemented as a StoreSubscriber because it also measures implementation details of the
// store (buckets etc), while StoreEvents work at a data-model level.

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct DataStoreRowStats {
    pub num_rows: u64,
    pub num_bytes: u64,
}

impl std::ops::Sub for DataStoreRowStats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            num_rows: self.num_rows - rhs.num_rows,
            num_bytes: self.num_bytes - rhs.num_bytes,
        }
    }
}

impl std::ops::Add for DataStoreRowStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            num_rows: self.num_rows + rhs.num_rows,
            num_bytes: self.num_bytes + rhs.num_bytes,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct DataStoreStats {
    pub type_registry: DataStoreRowStats,
    pub metadata_registry: DataStoreRowStats,

    /// `num_rows` is really `num_cells` in this case.
    pub static_tables: DataStoreRowStats,

    pub temporal: DataStoreRowStats,
    pub temporal_buckets: u64,

    pub total: DataStoreRowStats,
}

impl std::ops::Sub for DataStoreStats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            type_registry: self.type_registry - rhs.type_registry,
            metadata_registry: self.metadata_registry - rhs.metadata_registry,
            static_tables: self.static_tables - rhs.static_tables,
            temporal: self.temporal - rhs.temporal,
            temporal_buckets: self.temporal_buckets - rhs.temporal_buckets,
            total: self.total - rhs.total,
        }
    }
}

impl std::ops::Add for DataStoreStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            type_registry: self.type_registry + rhs.type_registry,
            metadata_registry: self.metadata_registry + rhs.metadata_registry,
            static_tables: self.static_tables + rhs.static_tables,
            temporal: self.temporal + rhs.temporal,
            temporal_buckets: self.temporal_buckets + rhs.temporal_buckets,
            total: self.total + rhs.total,
        }
    }
}

impl DataStoreStats {
    pub fn from_store(store: &DataStore) -> Self {
        re_tracing::profile_function!();

        let type_registry = {
            re_tracing::profile_scope!("type_registry");
            DataStoreRowStats {
                num_rows: store.type_registry.len() as _,
                num_bytes: store.type_registry.total_size_bytes(),
            }
        };

        let metadata_registry = {
            re_tracing::profile_scope!("metadata_registry");
            DataStoreRowStats {
                num_rows: store.metadata_registry.len() as _,
                num_bytes: store.metadata_registry.total_size_bytes(),
            }
        };

        let static_tables = {
            re_tracing::profile_scope!("static data");
            DataStoreRowStats {
                num_rows: store.num_static_rows(),
                num_bytes: store.static_size_bytes(),
            }
        };

        let (temporal, temporal_buckets) = {
            re_tracing::profile_scope!("temporal");
            (
                DataStoreRowStats {
                    num_rows: store.num_temporal_rows(),
                    num_bytes: store.temporal_size_bytes(),
                },
                store.num_temporal_buckets(),
            )
        };

        let total = DataStoreRowStats {
            num_rows: static_tables.num_rows + temporal.num_rows,
            num_bytes: type_registry.num_bytes
                + metadata_registry.num_bytes
                + static_tables.num_bytes
                + temporal.num_bytes,
        };

        Self {
            type_registry,
            metadata_registry,
            static_tables,
            temporal,
            temporal_buckets,
            total,
        }
    }

    /// Both static & temporal data.
    pub fn total_rows_and_bytes(&self) -> (u64, f64) {
        let mut num_rows = self.temporal.num_rows + self.metadata_registry.num_rows;
        let mut num_bytes = (self.temporal.num_bytes + self.metadata_registry.num_bytes) as f64;

        num_rows += self.static_tables.num_rows;
        num_bytes += self.static_tables.num_bytes as f64;

        (num_rows, num_bytes)
    }
}

// --- Data store ---

impl SizeBytes for MetadataRegistry<(TimePoint, EntityPathHash)> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.heap_size_bytes
    }
}

impl SizeBytes for DataStore {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.static_size_bytes() + self.temporal_size_bytes() // approximate
    }
}

impl DataStore {
    /// Returns the number of static rows stored across this entire store.
    #[inline]
    pub fn num_static_rows(&self) -> u64 {
        // A static table only ever contains a single row.
        self.static_tables.len() as _
    }

    /// Returns the size of the static data stored across this entire store.
    #[inline]
    pub fn static_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();
        self.static_tables
            .values()
            .map(|static_table| {
                static_table
                    .cells
                    .values()
                    .map(|static_cell| static_cell.cell.total_size_bytes())
                    .sum::<u64>()
            })
            .sum()
    }

    /// Returns the number of temporal index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its temporal indexed tables.
    #[inline]
    pub fn num_temporal_rows(&self) -> u64 {
        re_tracing::profile_function!();
        self.tables.values().map(|table| table.num_rows()).sum()
    }

    /// Returns the size of the temporal index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its temporal indexed tables, in bytes.
    #[inline]
    pub fn temporal_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();
        self.tables
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal indexed buckets stored across this entire store.
    #[inline]
    pub fn num_temporal_buckets(&self) -> u64 {
        re_tracing::profile_function!();
        self.tables.values().map(|table| table.num_buckets()).sum()
    }

    /// Stats for a specific entity path on a specific timeline
    pub fn entity_stats(
        &self,
        timeline: re_log_types::Timeline,
        entity_path_hash: re_log_types::EntityPathHash,
    ) -> EntityStats {
        let mut entity_stats = self.tables.get(&(entity_path_hash, timeline)).map_or(
            EntityStats::default(),
            |table| EntityStats {
                num_rows: table.buckets_num_rows,
                size_bytes: table.buckets_size_bytes,
                time_range: table.time_range(),
                num_static_cells: 0,
                static_size_bytes: 0,
            },
        );

        if let Some(static_table) = self.static_tables.get(&entity_path_hash) {
            entity_stats.num_static_cells = static_table.cells.len() as _;
            entity_stats.static_size_bytes = static_table
                .cells
                .values()
                .map(|static_cell| static_cell.cell.total_size_bytes())
                .sum();
        }

        entity_stats
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityStats {
    /// Number of rows in the table.
    pub num_rows: u64,

    /// Approximate number of bytes used.
    pub size_bytes: u64,

    /// The time covered by the entity.
    pub time_range: re_log_types::TimeRange,

    /// Number of static cells.
    pub num_static_cells: u64,

    /// Approximate number of bytes used for static data.
    pub static_size_bytes: u64,
}

impl Default for EntityStats {
    fn default() -> Self {
        Self {
            num_rows: 0,
            size_bytes: 0,
            time_range: re_log_types::TimeRange::EMPTY,
            num_static_cells: 0,
            static_size_bytes: 0,
        }
    }
}

// --- Temporal ---

impl IndexedTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    #[inline]
    pub fn num_rows(&self) -> u64 {
        self.buckets_num_rows
    }

    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    ///
    /// Recomputed from scratch, for sanity checking.
    #[inline]
    pub(crate) fn num_rows_uncached(&self) -> u64 {
        re_tracing::profile_function!();
        self.buckets.values().map(|bucket| bucket.num_rows()).sum()
    }

    #[inline]
    pub(crate) fn size_bytes_uncached(&self) -> u64 {
        re_tracing::profile_function!();
        self.stack_size_bytes()
            + self
                .buckets
                .values()
                .map(|bucket| bucket.total_size_bytes())
                .sum::<u64>()
    }

    /// Returns the number of buckets stored across this entire table.
    #[inline]
    pub fn num_buckets(&self) -> u64 {
        self.buckets.len() as _
    }

    /// The time range covered by this table.
    pub fn time_range(&self) -> TimeRange {
        if let (Some((_, first)), Some((_, last))) = (
            self.buckets.first_key_value(),
            self.buckets.last_key_value(),
        ) {
            first
                .inner
                .read()
                .time_range
                .union(last.inner.read().time_range)
        } else {
            TimeRange::EMPTY
        }
    }
}

impl SizeBytes for IndexedTable {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.buckets_size_bytes
    }
}

impl IndexedBucket {
    /// Returns the number of rows stored across this bucket.
    #[inline]
    pub fn num_rows(&self) -> u64 {
        self.inner.read().col_time.len() as u64
    }
}

impl SizeBytes for IndexedBucket {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.inner.read().size_bytes
    }
}

impl IndexedBucketInner {
    /// Computes and caches the size of both the control & component data stored in this bucket,
    /// stack and heap included, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, …).
    #[inline]
    pub fn compute_size_bytes(&mut self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            max_row_id,
            columns,
            size_bytes,
        } = self;

        *size_bytes = is_sorted.total_size_bytes()
            + time_range.total_size_bytes()
            + col_time.total_size_bytes()
            + col_insert_id.total_size_bytes()
            + col_row_id.total_size_bytes()
            + max_row_id.total_size_bytes()
            + columns.total_size_bytes()
            + size_bytes.total_size_bytes();

        *size_bytes
    }
}
