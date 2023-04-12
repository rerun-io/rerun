use nohash_hasher::IntMap;
use re_log_types::{ComponentName, DataCellColumn};

use crate::{
    store::IndexedBucketInner, DataStore, DataStoreConfig, IndexedBucket, IndexedTable,
    PersistentIndexedTable,
};

// ---

#[derive(Default, Debug, Clone)]
pub struct DataStoreStats {
    pub total_timeless_rows: u64,
    pub total_timeless_size_bytes: u64,

    pub total_temporal_rows: u64,
    pub total_temporal_size_bytes: u64,
    pub total_temporal_buckets: u64,

    pub total_rows: u64,
    pub total_size_bytes: u64,

    pub config: DataStoreConfig,
}

impl DataStoreStats {
    pub fn from_store(store: &DataStore) -> Self {
        crate::profile_function!();

        let total_timeless_rows = store.total_timeless_rows();
        let total_timeless_size_bytes = store.total_timeless_size_bytes();

        let total_temporal_rows = store.total_temporal_rows();
        let total_temporal_size_bytes = store.total_temporal_size_bytes();
        let total_temporal_buckets = store.total_temporal_buckets();

        let total_rows = total_timeless_rows + total_temporal_rows;
        let total_size_bytes = total_timeless_size_bytes + total_temporal_size_bytes;

        Self {
            total_timeless_rows,
            total_timeless_size_bytes,

            total_temporal_rows,
            total_temporal_size_bytes,
            total_temporal_buckets,

            total_rows,
            total_size_bytes,

            config: store.config.clone(),
        }
    }
}

// --- Data store ---

impl DataStore {
    /// Returns the number of timeless index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its timeless indexed tables.
    #[inline]
    pub fn total_timeless_rows(&self) -> u64 {
        crate::profile_function!();
        self.timeless_tables
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its timeless indexed tables, in bytes.
    #[inline]
    pub fn total_timeless_size_bytes(&self) -> u64 {
        crate::profile_function!();
        self.timeless_tables
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its temporal indexed tables.
    #[inline]
    pub fn total_temporal_rows(&self) -> u64 {
        crate::profile_function!();
        self.tables.values().map(|table| table.total_rows()).sum()
    }

    /// Returns the size of the temporal index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its temporal indexed tables, in bytes.
    #[inline]
    pub fn total_temporal_size_bytes(&self) -> u64 {
        crate::profile_function!();
        self.tables
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal indexed buckets stored across this entire store.
    #[inline]
    pub fn total_temporal_buckets(&self) -> u64 {
        crate::profile_function!();
        self.tables
            .values()
            .map(|table| table.total_buckets())
            .sum()
    }
}

// --- Temporal ---

impl IndexedTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    #[inline]
    pub fn total_rows(&self) -> u64 {
        self.buckets_num_rows
    }

    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    ///
    /// Recomputed from scratch, for sanity checking.
    #[inline]
    pub(crate) fn total_rows_uncached(&self) -> u64 {
        crate::profile_function!();
        self.buckets.values().map(|bucket| bucket.num_rows()).sum()
    }

    /// The size of both the control & component data stored in this table, across all of its
    /// buckets, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, ...).
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        crate::profile_function!();

        let Self {
            timeline,
            ent_path,
            cluster_key,
            buckets: _,
            all_components,
            buckets_num_rows: _,
            buckets_size_bytes,
        } = self;

        let size_bytes = std::mem::size_of_val(timeline)
            + std::mem::size_of_val(ent_path)
            + std::mem::size_of_val(cluster_key)
            + (all_components.len() * std::mem::size_of::<ComponentName>());

        size_bytes as u64 + buckets_size_bytes
    }

    /// The size of both the control & component data stored in this table, across all of its
    /// buckets, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, ...).
    ///
    /// Recomputed from scratch, for sanity checking.
    #[inline]
    pub(crate) fn total_size_bytes_uncached(&self) -> u64 {
        crate::profile_function!();

        let Self {
            timeline,
            ent_path,
            cluster_key,
            buckets,
            all_components,
            buckets_num_rows: _,
            buckets_size_bytes: _,
        } = self;

        let buckets_size_bytes = buckets
            .values()
            .map(|bucket| bucket.size_bytes())
            .sum::<u64>();

        let size_bytes = std::mem::size_of_val(timeline)
            + std::mem::size_of_val(ent_path)
            + std::mem::size_of_val(cluster_key)
            + (all_components.len() * std::mem::size_of::<ComponentName>());

        size_bytes as u64 + buckets_size_bytes
    }

    /// Returns the number of buckets stored across this entire table.
    #[inline]
    pub fn total_buckets(&self) -> u64 {
        self.buckets.len() as _
    }
}

impl IndexedBucket {
    /// Returns the number of rows stored across this bucket.
    #[inline]
    pub fn num_rows(&self) -> u64 {
        crate::profile_function!();
        self.inner.read().col_time.len() as u64
    }

    /// The size of both the control & component data stored in this bucket, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, ...).
    #[inline]
    pub fn size_bytes(&self) -> u64 {
        crate::profile_function!();

        let Self {
            timeline,
            cluster_key,
            inner,
        } = self;

        (std::mem::size_of_val(timeline) + std::mem::size_of_val(cluster_key)) as u64
            + inner.read().size_bytes
    }
}

impl IndexedBucketInner {
    /// Computes and caches the size of both the control & component data stored in this bucket,
    /// in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, ...).
    #[inline]
    pub fn compute_size_bytes(&mut self) -> u64 {
        crate::profile_function!();

        let Self {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            size_bytes,
        } = self;

        let control_size_bytes = std::mem::size_of_val(is_sorted)
            + std::mem::size_of_val(time_range)
            + std::mem::size_of_val(col_time.as_slice())
            + std::mem::size_of_val(col_insert_id.as_slice())
            + std::mem::size_of_val(col_row_id.as_slice())
            + std::mem::size_of_val(col_num_instances.as_slice())
            + std::mem::size_of_val(size_bytes);

        let data_size_bytes = compute_columns_size_bytes(columns);

        *size_bytes = control_size_bytes as u64 + data_size_bytes;

        *size_bytes
    }
}

// --- Timeless ---

impl PersistentIndexedTable {
    /// Returns the number of rows stored across this table.
    #[inline]
    pub fn total_rows(&self) -> u64 {
        self.col_num_instances.len() as _
    }

    /// The size of both the control & component data stored in this table, in bytes.
    ///
    /// This is a best-effort approximation, adequate for most purposes (stats,
    /// triggering GCs, ...).
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        crate::profile_function!();

        let Self {
            ent_path,
            cluster_key,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        } = self;

        let control_size_bytes = std::mem::size_of_val(ent_path)
            + std::mem::size_of_val(cluster_key)
            + std::mem::size_of_val(col_insert_id.as_slice())
            + std::mem::size_of_val(col_row_id.as_slice())
            + std::mem::size_of_val(col_num_instances.as_slice());

        let data_size_bytes = compute_columns_size_bytes(columns);

        control_size_bytes as u64 + data_size_bytes
    }
}

// --- Common ---

/// Computes the size in bytes of an entire table's worth of arrow data.
fn compute_columns_size_bytes(columns: &IntMap<ComponentName, DataCellColumn>) -> u64 {
    crate::profile_function!();
    let keys = (columns.keys().len() * std::mem::size_of::<ComponentName>()) as u64;
    let cells = columns
        .values()
        .flat_map(|column| column.iter())
        .flatten() // option
        .map(|cell| cell.size_bytes())
        .sum::<u64>();
    keys + cells
}

#[test]
fn compute_table_size_bytes_ignore_headers() {
    let columns = Default::default();
    assert_eq!(0, compute_columns_size_bytes(&columns));
}
