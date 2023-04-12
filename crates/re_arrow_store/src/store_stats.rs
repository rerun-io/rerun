use crate::{DataStore, DataStoreConfig, IndexedBucket, IndexedTable, PersistentIndexedTable};

// ---

// TODO(cmc): compute incrementally once/if this becomes too expensive.
#[derive(Default, Debug)]
pub struct DataStoreStats {
    pub total_timeless_index_rows: u64,
    pub total_timeless_index_size_bytes: u64,

    pub total_temporal_index_rows: u64,
    pub total_temporal_index_size_bytes: u64,
    pub total_temporal_index_buckets: u64,

    pub total_index_rows: u64,
    pub total_index_size_bytes: u64,

    pub config: DataStoreConfig,
}

impl DataStoreStats {
    pub fn from_store(store: &DataStore) -> Self {
        crate::profile_function!();

        let total_timeless_index_rows = store.total_timeless_index_rows();
        let total_timeless_index_size_bytes = store.total_timeless_index_size_bytes();

        let total_temporal_index_rows = store.total_temporal_index_rows();
        let total_temporal_index_size_bytes = store.total_temporal_index_size_bytes();
        let total_temporal_index_buckets = store.total_temporal_index_buckets();

        let total_index_rows = total_timeless_index_rows + total_temporal_index_rows;
        let total_index_size_bytes =
            total_timeless_index_size_bytes + total_temporal_index_size_bytes;

        Self {
            total_timeless_index_rows,
            total_timeless_index_size_bytes,

            total_temporal_index_rows,
            total_temporal_index_size_bytes,
            total_temporal_index_buckets,

            total_index_rows,
            total_index_size_bytes,

            config: store.config.clone(),
        }
    }
}

// --- Data store ---

impl DataStore {
    /// Returns the number of timeless index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its timeless indexed tables.
    #[inline]
    pub fn total_timeless_index_rows(&self) -> u64 {
        crate::profile_function!();
        self.timeless_tables
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its timeless indexed tables, in bytes.
    #[inline]
    pub fn total_timeless_index_size_bytes(&self) -> u64 {
        crate::profile_function!();
        self.timeless_tables
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its temporal indexed tables.
    #[inline]
    pub fn total_temporal_index_rows(&self) -> u64 {
        crate::profile_function!();
        self.tables.values().map(|table| table.total_rows()).sum()
    }

    /// Returns the size of the temporal index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its temporal indexed tables, in bytes.
    #[inline]
    pub fn total_temporal_index_size_bytes(&self) -> u64 {
        crate::profile_function!();
        self.tables
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal indexed buckets stored across this entire store.
    #[inline]
    pub fn total_temporal_index_buckets(&self) -> u64 {
        crate::profile_function!();
        self.tables
            .values()
            .map(|table| table.total_buckets())
            .sum()
    }
}

// --- Persistent Indices ---

impl PersistentIndexedTable {
    /// Returns the number of rows stored across this table.
    #[inline]
    pub fn total_rows(&self) -> u64 {
        self.col_num_instances.len() as _
    }

    /// Returns the size of the data stored across this table, in bytes.
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }
}

// --- Indices ---

impl IndexedTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    #[inline]
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Returns the size of data stored across this entire table, i.e. the sum of the size of
    /// the data stored across all of its buckets, in bytes.
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
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
    pub fn total_rows(&self) -> u64 {
        self.inner.read().col_time.len() as u64
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    #[inline]
    pub fn total_size_bytes(&self) -> u64 {
        self.inner.read().total_size_bytes
    }
}
