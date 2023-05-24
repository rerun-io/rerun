use ahash::HashSetExt;
use re_log_types::{RowId, SizeBytes as _, Time, TimeInt, TimeRange};

use crate::{
    store::{IndexedBucketInner, IndexedTable},
    DataStore, DataStoreStats,
};

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given fraction.
    ///
    /// The fraction must be a float in the range [0.0 : 1.0].
    DropAtLeastFraction(f64),
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                write!(f, "DropAtLeast({:.3}%)", re_format::format_f64(*p * 100.0))
            }
        }
    }
}

impl DataStore {
    /// Triggers a garbage collection according to the desired `target`.
    ///
    /// Garbage collection's performance is bounded by the number of buckets in each table (for
    /// each `RowId`, we have to find the corresponding bucket, which is roughly `O(log(n))`) as
    /// well as the number of rows in each of those buckets (for each `RowId`, we have to sort the
    /// corresponding bucket (roughly `O(n*log(n))`) and then find the corresponding row (roughly
    /// `O(log(n))`.
    /// The size of the data itself has no impact on performance.
    ///
    /// Returns the list of `RowId`s that were purged from the store.
    ///
    /// ## Semantics
    ///
    /// Garbage collection works on a row-level basis and is driven by [`RowId`] order,
    /// i.e. the order defined by the clients' wall-clocks, allowing it to drop data across
    /// the different timelines
    /// in a fair, deterministic manner.
    /// Similarly, out-of-order data is supported out of the box.
    ///
    /// The garbage collector doesn't deallocate data in and of itself: all it does is drop the
    /// store's internal references to that data (the `DataCell`s), which will be deallocated once
    /// their reference count reaches 0.
    ///
    /// ## Limitations
    ///
    /// The garbage collector is currently unaware of our latest-at semantics, i.e. it will drop
    /// old data even if doing so would impact the results of recent queries.
    /// See <https://github.com/rerun-io/rerun/issues/1803>.
    //
    // TODO(#1804): There shouldn't be any need to return the purged `RowId`s, all secondary
    // datastructures should be able to purge themselves based solely off of
    // [`DataStore::oldest_time_per_timeline`].
    //
    // TODO(#1803): The GC should be aware of latest-at semantics and make sure they are upheld
    // when purging data.
    //
    // TODO(#1823): Workload specific optimizations.
    pub fn gc(&mut self, target: GarbageCollectionTarget) -> (Vec<RowId>, DataStoreStats) {
        crate::profile_function!();

        self.gc_id += 1;

        // NOTE: only temporal data and row metadata get purged!
        let stats_before = DataStoreStats::from_store(self);
        let initial_num_rows =
            stats_before.temporal.num_rows + stats_before.metadata_registry.num_rows;
        let initial_num_bytes =
            (stats_before.temporal.num_bytes + stats_before.metadata_registry.num_bytes) as f64;

        let row_ids = match target {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                assert!((0.0..=1.0).contains(&p));

                let num_bytes_to_drop = initial_num_bytes * p;
                let target_num_bytes = initial_num_bytes - num_bytes_to_drop;

                re_log::debug!(
                    kind = "gc",
                    id = self.gc_id,
                    %target,
                    initial_num_rows = re_format::format_large_number(initial_num_rows as _),
                    initial_num_bytes = re_format::format_bytes(initial_num_bytes),
                    target_num_bytes = re_format::format_bytes(target_num_bytes),
                    drop_at_least_num_bytes = re_format::format_bytes(num_bytes_to_drop),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(num_bytes_to_drop)
            }
        };

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        // NOTE: only temporal data and row metadata get purged!
        let stats_after = DataStoreStats::from_store(self);
        let new_num_rows = stats_after.temporal.num_rows + stats_after.metadata_registry.num_rows;
        let new_num_bytes =
            (stats_after.temporal.num_bytes + stats_after.metadata_registry.num_bytes) as f64;

        re_log::debug!(
            kind = "gc",
            id = self.gc_id,
            %target,
            initial_num_rows = re_format::format_large_number(initial_num_rows as _),
            initial_num_bytes = re_format::format_bytes(initial_num_bytes),
            new_num_rows = re_format::format_large_number(new_num_rows as _),
            new_num_bytes = re_format::format_bytes(new_num_bytes),
            "GC done"
        );

        let stats_diff = stats_before - stats_after;

        (row_ids, stats_diff)
    }

    /// Tries to drop _at least_ `num_bytes_to_drop` bytes of data from the store.
    ///
    /// Returns the list of `RowId`s that were purged from the store.
    fn gc_drop_at_least_num_bytes(&mut self, mut num_bytes_to_drop: f64) -> Vec<RowId> {
        crate::profile_function!();

        let mut row_ids = Vec::new();

        // The algorithm is straightforward:
        // 1. Pop the oldest `RowId` available
        // 2. Find all tables that potentially hold data associated with that `RowId`
        // 3. Drop the associated row and account for the space we got back
        while num_bytes_to_drop > 0.0 {
            // pop next row id
            let Some((row_id, timepoint)) = self.metadata_registry.pop_first() else {
                break;
            };
            let metadata_dropped_size_bytes =
                row_id.total_size_bytes() + timepoint.total_size_bytes();
            self.metadata_registry.heap_size_bytes -= metadata_dropped_size_bytes;
            num_bytes_to_drop -= metadata_dropped_size_bytes as f64;
            row_ids.push(row_id);

            // find all tables that could possibly contain this `RowId`
            let tables = self.tables.iter_mut().filter_map(|((timeline, _), table)| {
                timepoint.get(timeline).map(|time| (*time, table))
            });

            for (time, table) in tables {
                num_bytes_to_drop -= table.try_drop_row(row_id, time.as_i64()) as f64;
            }
        }

        row_ids
    }

    pub fn gc_drop_by_cutoff_time(&mut self, cutoff_time: i64) -> ahash::HashSet<RowId> {
        let mut row_ids = ahash::HashSet::new();

        for (_, table) in &mut self.tables.iter_mut() {
            let mut row_ids_to_remove = Vec::new();
            {
                let (_, bucket) = table.find_bucket(cutoff_time.into());
                for row_id in bucket.inner.write().col_row_id.iter() {
                    for time in self.metadata_registry.get(row_id).unwrap().times() {
                        if time.as_i64() < cutoff_time {
                            row_ids_to_remove.push((*row_id, time));
                            if !row_ids.contains(row_id) {
                                row_ids.insert(*row_id);
                            }
                        }
                    }
                }
            }
            for (row_id, time) in row_ids_to_remove {
                table.try_drop_row(row_id, time.as_i64());
            }
        }
        row_ids
    }
}

impl IndexedTable {
    /// Tries to drop the given `row_id` from the table, which is expected to be found at the
    /// specified `time`.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(&mut self, row_id: RowId, time: i64) -> u64 {
        crate::profile_function!();

        let table_has_more_than_one_bucket = self.buckets.len() > 1;

        let (bucket_key, bucket) = self.find_bucket_mut(time.into());
        let bucket_num_bytes = bucket.total_size_bytes();

        let mut dropped_num_bytes = {
            let inner = &mut *bucket.inner.write();
            inner.try_drop_row(row_id, time)
        };

        // NOTE: We always need to keep at least one bucket alive, otherwise we have
        // nowhere to write to.
        if table_has_more_than_one_bucket && bucket.num_rows() == 0 {
            // NOTE: We're dropping the bucket itself in this case, rather than just its
            // contents.
            debug_assert!(
                dropped_num_bytes <= bucket_num_bytes,
                "Bucket contained more bytes than it thought"
            );
            dropped_num_bytes = bucket_num_bytes;
            self.buckets.remove(&bucket_key);

            // NOTE: If this is the first bucket of the table that we've just removed, we need the
            // next one to become responsible for `-∞`.
            if bucket_key == TimeInt::MIN {
                if let Some((_, bucket)) = self.buckets.pop_first() {
                    self.buckets.insert(TimeInt::MIN, bucket);
                }
            }
        }

        self.buckets_size_bytes -= dropped_num_bytes;
        self.buckets_num_rows -= (dropped_num_bytes > 0) as u64;

        dropped_num_bytes
    }
}

impl IndexedBucketInner {
    /// Tries to drop the given `row_id` from the table, which is expected to be found at the
    /// specified `time`.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(&mut self, row_id: RowId, time: i64) -> u64 {
        crate::profile_function!();

        self.sort();

        let IndexedBucketInner {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            size_bytes,
        } = self;

        let mut dropped_num_bytes = 0u64;

        let mut row_index = col_time.partition_point(|&time2| time2 < time);
        while col_time.get(row_index) == Some(&time) {
            if col_row_id[row_index] != row_id {
                row_index += 1;
                continue;
            }

            // Update the time_range min/max:
            if col_time.len() == 1 {
                // We removed the last row
                *time_range = TimeRange::EMPTY;
            } else {
                *is_sorted = false;

                // We have at least two rows, so we can safely [index] here:
                if row_index == 0 {
                    // We removed the first row, so the second row holds the new min
                    time_range.min = col_time[1].into();
                }
                if row_index + 1 == col_time.len() {
                    // We removed the last row, so the penultimate row holds the new max
                    time_range.max = col_time[row_index - 1].into();
                }
            }

            // col_row_id
            let removed_row_id = col_row_id.swap_remove(row_index);
            debug_assert_eq!(row_id, removed_row_id);
            dropped_num_bytes += removed_row_id.total_size_bytes();

            // col_time
            let row_time = col_time.swap_remove(row_index);
            dropped_num_bytes += row_time.total_size_bytes();

            // col_insert_id (if present)
            if !col_insert_id.is_empty() {
                dropped_num_bytes += col_insert_id.swap_remove(row_index).total_size_bytes();
            }

            // col_num_instances
            dropped_num_bytes += col_num_instances.swap_remove(row_index).total_size_bytes();

            // each data column
            for column in columns.values_mut() {
                dropped_num_bytes += column.0.swap_remove(row_index).total_size_bytes();
            }

            // NOTE: A single `RowId` cannot possibly have more than one datapoint for
            // a single timeline.
            break;
        }

        *size_bytes -= dropped_num_bytes;

        dropped_num_bytes
    }
}
