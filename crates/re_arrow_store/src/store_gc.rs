use ahash::{HashMap, HashSet};

use re_log_types::{RowId, SizeBytes as _, TimeInt, TimeRange};
use re_types::ComponentName;

use crate::{
    store::{IndexedBucketInner, IndexedTable, PersistentIndexedTable},
    DataStore, DataStoreStats,
};

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given fraction.
    ///
    /// The fraction must be a float in the range [0.0 : 1.0].
    DropAtLeastFraction(f64),

    /// GC Everything that isn't protected
    Everything,
}

#[derive(Debug, Clone, Copy)]
pub struct GarbageCollectionOptions {
    /// What target threshold should the GC try to meet.
    pub target: GarbageCollectionTarget,

    /// Whether to also GC timeless data.
    pub gc_timeless: bool,

    /// How many component revisions to preserve on each timeline.
    pub protect_latest: usize,

    /// Whether to purge tables that no longer contain any data
    pub purge_empty_tables: bool,
}

impl GarbageCollectionOptions {
    pub fn gc_everything() -> Self {
        GarbageCollectionOptions {
            target: GarbageCollectionTarget::Everything,
            gc_timeless: true,
            protect_latest: 0,
            purge_empty_tables: true,
        }
    }
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                write!(f, "DropAtLeast({:.3}%)", re_format::format_f64(*p * 100.0))
            }
            GarbageCollectionTarget::Everything => write!(f, "Everything"),
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
    /// The garbage collector has limited support for latest-at semantics. The configuration option:
    /// [`GarbageCollectionOptions::protect_latest`] will protect the N latest values of each
    /// component on each timeline. The only practical guarantee this gives is that a latest-at query
    /// with a value of max-int will be be unchanged. However, latest-at queries from other arbitrary
    /// points in time may provide different results pre- and post- GC.
    ///
    /// NOTE: This configuration option is not yet enabled for the Rerun viewer GC pass.
    ///
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
    pub fn gc(&mut self, options: GarbageCollectionOptions) -> (Vec<RowId>, DataStoreStats) {
        re_tracing::profile_function!();

        self.gc_id += 1;

        let stats_before = DataStoreStats::from_store(self);

        let (initial_num_rows, initial_num_bytes) =
            stats_before.total_rows_and_bytes_with_timeless(options.gc_timeless);

        let protected_rows = self.find_all_protected_rows(options.protect_latest);

        let mut row_ids = match options.target {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                assert!((0.0..=1.0).contains(&p));

                let num_bytes_to_drop = initial_num_bytes * p;
                let target_num_bytes = initial_num_bytes - num_bytes_to_drop;

                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    initial_num_rows = re_format::format_large_number(initial_num_rows as _),
                    initial_num_bytes = re_format::format_bytes(initial_num_bytes),
                    target_num_bytes = re_format::format_bytes(target_num_bytes),
                    drop_at_least_num_bytes = re_format::format_bytes(num_bytes_to_drop),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(
                    num_bytes_to_drop,
                    options.gc_timeless,
                    &protected_rows,
                )
            }
            GarbageCollectionTarget::Everything => {
                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    initial_num_rows = re_format::format_large_number(initial_num_rows as _),
                    initial_num_bytes = re_format::format_bytes(initial_num_bytes),
                    "starting GC"
                );

                self.gc_everything(options.gc_timeless, &protected_rows)
            }
        };

        if options.purge_empty_tables {
            row_ids.extend(self.purge_empty_tables());
        }

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        // NOTE: only temporal data and row metadata get purged!
        let stats_after = DataStoreStats::from_store(self);
        let (new_num_rows, new_num_bytes) =
            stats_after.total_rows_and_bytes_with_timeless(options.gc_timeless);

        re_log::trace!(
            kind = "gc",
            id = self.gc_id,
            %options.target,
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
    fn gc_drop_at_least_num_bytes(
        &mut self,
        mut num_bytes_to_drop: f64,
        include_timeless: bool,
        protected_rows: &HashSet<RowId>,
    ) -> Vec<RowId> {
        re_tracing::profile_function!();

        let mut row_ids = Vec::new();

        // The algorithm is straightforward:
        // 1. Find the the oldest `RowId` that is not protected
        // 2. Find all tables that potentially hold data associated with that `RowId`
        // 3. Drop the associated row and account for the space we got back

        let mut candidate_rows = self.metadata_registry.registry.iter();

        while num_bytes_to_drop > 0.0 {
            // Try to get the next candidate
            let Some((row_id, timepoint)) = candidate_rows.next() else {
                break;
            };
            if protected_rows.contains(row_id) {
                continue;
            }
            let metadata_dropped_size_bytes =
                row_id.total_size_bytes() + timepoint.total_size_bytes();
            self.metadata_registry.heap_size_bytes -= metadata_dropped_size_bytes;
            num_bytes_to_drop -= metadata_dropped_size_bytes as f64;
            row_ids.push(*row_id);

            // find all tables that could possibly contain this `RowId`
            let temporal_tables = self.tables.iter_mut().filter_map(|((timeline, _), table)| {
                timepoint.get(timeline).map(|time| (*time, table))
            });

            for (time, table) in temporal_tables {
                num_bytes_to_drop -= table.try_drop_row(*row_id, time.as_i64()) as f64;
            }

            // TODO(jleibs): This is a worst-case removal-order. Would be nice to collect all the rows
            // first and then remove them in one pass.
            if timepoint.is_timeless() && include_timeless {
                for table in self.timeless_tables.values_mut() {
                    num_bytes_to_drop -= table.try_drop_row(*row_id) as f64;
                }
            }
        }

        // Purge the removed rows from the metadata_registry
        for row_id in &row_ids {
            self.metadata_registry.remove(row_id);
        }

        // Any tables that are empty can be dropped
        self.tables.retain(|_, table| table.num_rows() > 0);
        self.timeless_tables.retain(|_, table| table.num_rows() > 0);

        row_ids
    }

    /// GCs everything that isn't protected.
    ///
    /// Returns the list of `RowId`s that were purged from the store.
    fn gc_everything(
        &mut self,
        include_timeless: bool,
        protected_rows: &HashSet<RowId>,
    ) -> Vec<RowId> {
        re_tracing::profile_function!();

        let mut row_ids = Vec::new();

        // Iterate from newest to oldest rows since it generally preserves sorting
        // and makes dropping cheaper
        for (row_id, timepoint) in self.metadata_registry.registry.iter().rev() {
            if protected_rows.contains(row_id) {
                continue;
            }
            let metadata_dropped_size_bytes =
                row_id.total_size_bytes() + timepoint.total_size_bytes();
            self.metadata_registry.heap_size_bytes -= metadata_dropped_size_bytes;

            row_ids.push(*row_id);

            // find all tables that could possibly contain this `RowId`
            let temporal_tables = self.tables.iter_mut().filter_map(|((timeline, _), table)| {
                timepoint.get(timeline).map(|time| (*time, table))
            });

            for (time, table) in temporal_tables {
                table.try_drop_row(*row_id, time.as_i64());
            }

            if timepoint.is_timeless() && include_timeless {
                for table in self.timeless_tables.values_mut() {
                    table.try_drop_row(*row_id);
                }
            }
        }

        // Purge the removed rows from the metadata_registry
        for row_id in &row_ids {
            self.metadata_registry.remove(row_id);
        }

        // Any tables that are empty can be dropped
        self.tables.retain(|_, table| table.num_rows() > 0);
        self.timeless_tables.retain(|_, table| table.num_rows() > 0);

        row_ids
    }

    /// For each `EntityPath`, `Timeline`, `Component` find the N latest [`RowId`]s.
    ///
    /// These are the rows that must be protected so as not to impact a latest-at query.
    /// Note that latest for Timeless is currently based on insertion-order rather than
    /// tuid. [See: #1807](https://github.com/rerun-io/rerun/issues/1807)
    //
    // TODO(jleibs): More complex functionality might required expanding this to also
    // *ignore* specific entities, components, timelines, etc. for this protection.
    //
    // TODO(jleibs): `RowId`s should never overlap between entities. Creating a single large
    // HashSet might actually be sub-optimal here. Consider switching to a map of
    // `EntityPath` -> `HashSet<RowId>`.
    fn find_all_protected_rows(&mut self, target_count: usize) -> HashSet<RowId> {
        re_tracing::profile_function!();

        if target_count == 0 {
            return Default::default();
        }

        // We need to sort to be able to determine latest-at.
        self.sort_indices_if_needed();

        let mut protected_rows: HashSet<RowId> = Default::default();

        // Find all protected rows in regular indexed tables
        for table in self.tables.values() {
            let mut components_to_find: HashMap<ComponentName, usize> = table
                .all_components
                .iter()
                .filter(|c| **c != table.cluster_key)
                .map(|c| (*c, target_count))
                .collect();

            for bucket in table.buckets.values().rev() {
                for (component, count) in &mut components_to_find {
                    if *count == 0 {
                        continue;
                    }
                    let inner = bucket.inner.read();
                    // TODO(jleibs): If the entire column for a component is empty, we should
                    // make sure the column is dropped so we don't have to iterate over a
                    // bunch of Nones.
                    if let Some(column) = inner.columns.get(component) {
                        for row in column
                            .iter()
                            .enumerate()
                            .rev()
                            .filter_map(|(row_index, cell)| {
                                cell.as_ref().and_then(|_| inner.col_row_id.get(row_index))
                            })
                            .take(*count)
                        {
                            *count -= 1;
                            protected_rows.insert(*row);
                        }
                    }
                }
            }
        }

        // Find all protected rows in timeless tables
        // NOTE this is still based on insertion order.
        // https://github.com/rerun-io/rerun/issues/1807
        for table in self.timeless_tables.values() {
            let mut components_to_find: HashMap<ComponentName, usize> = table
                .columns
                .keys()
                .filter(|c| **c != table.cluster_key)
                .map(|c| (*c, target_count))
                .collect();

            for (component, count) in &mut components_to_find {
                if *count == 0 {
                    continue;
                }
                // TODO(jleibs): If the entire column for a component is empty, we should
                // make sure the column is dropped so we don't have to iterate over a
                // bunch of Nones.
                if let Some(column) = table.columns.get(component) {
                    for row_id in column
                        .iter()
                        .enumerate()
                        .rev()
                        .filter_map(|(row_index, cell)| {
                            cell.as_ref().and_then(|_| table.col_row_id.get(row_index))
                        })
                        .take(*count)
                    {
                        *count -= 1;
                        protected_rows.insert(*row_id);
                    }
                }
            }
        }

        protected_rows
    }

    /// Remove any tables which contain only components which are empty.
    // TODO(jleibs): We could optimize this further by also erasing empty columns.
    fn purge_empty_tables(&mut self) -> Vec<RowId> {
        re_tracing::profile_function!();

        let mut row_ids = Vec::new();

        // Drop any empty timeless tables
        self.timeless_tables.retain(|_, table| {
            // If any column is non-empty, we need to keep this table
            for num in &table.col_num_instances {
                if num != &0 {
                    return true;
                }
            }

            // Otherwise we can drop it
            row_ids.extend(table.col_row_id.iter());
            false
        });

        // Drop any empty temporal tables that aren't backed by a timeless table
        self.tables.retain(|(_, entity), table| {
            // If the timeless table still exists, this table might be storing empty values
            // that hide the timeless values, so keep it around.
            if self.timeless_tables.contains_key(entity) {
                return true;
            }

            // If any bucket has a non-empty component in any column, we keep it.
            for bucket in table.buckets.values() {
                let inner = bucket.inner.read();
                for num in &inner.col_num_instances {
                    if num != &0 {
                        return true;
                    }
                }
            }

            // Otherwise we can drop it
            for bucket in table.buckets.values() {
                let inner = bucket.inner.read();
                row_ids.extend(inner.col_row_id.iter());
            }
            false
        });

        row_ids
    }
}

impl IndexedTable {
    /// Tries to drop the given `row_id` from the table, which is expected to be found at the
    /// specified `time`.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(&mut self, row_id: RowId, time: i64) -> u64 {
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

impl PersistentIndexedTable {
    /// Tries to drop the given `row_id` from the table.
    ///
    /// Returns how many bytes were actually dropped, or zero if the row wasn't found.
    fn try_drop_row(&mut self, row_id: RowId) -> u64 {
        let mut dropped_num_bytes = 0u64;

        let PersistentIndexedTable {
            ent_path: _,
            cluster_key: _,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        } = self;

        // TODO(jleibs) Timeless data isn't sorted, so we need to do a full scan here.
        // Speed this up when we implement: https://github.com/rerun-io/rerun/issues/1807
        if let Some(row_index) = col_row_id
            .iter()
            .enumerate()
            .find(|(_, r)| **r == row_id)
            .map(|(index, _)| index)
        {
            // col_row_id
            // TODO(jleibs) Use swap_remove once we have a notion of sorted
            let removed_row_id = col_row_id.remove(row_index);
            debug_assert_eq!(row_id, removed_row_id);
            dropped_num_bytes += removed_row_id.total_size_bytes();

            // col_insert_id (if present)
            if !col_insert_id.is_empty() {
                // TODO(jleibs) Use swap_remove once we have a notion of sorted
                dropped_num_bytes += col_insert_id.remove(row_index).total_size_bytes();
            }

            // col_num_instances
            // TODO(jleibs) Use swap_remove once we have a notion of sorted
            dropped_num_bytes += col_num_instances.remove(row_index).total_size_bytes();

            // each data column
            for column in columns.values_mut() {
                dropped_num_bytes += column.0.remove(row_index).total_size_bytes();
            }
        }

        dropped_num_bytes
    }
}
