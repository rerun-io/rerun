use arrow2::datatypes::DataType;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use parking_lot::RwLock;
use smallvec::SmallVec;

use re_log::{debug, trace};
use re_log_types::{
    component_types::InstanceKey, ComponentName, DataCell, DataCellColumn, DataCellError, DataRow,
    DataTable, RowId, SizeBytes as _, TimeInt, TimePoint, TimeRange,
};

use crate::{
    store::MetadataRegistry, DataStore, DataStoreConfig, IndexedBucket, IndexedBucketInner,
    IndexedTable, PersistentIndexedTable,
};

// TODO(cmc): the store should insert column-per-column rather than row-per-row (purely a
// performance matter).

// --- Data store ---

#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    #[error("Error with one or more the underlying data cells")]
    DataCell(#[from] DataCellError),

    #[error("The cluster component must be dense, got {0:?}")]
    SparseClusteringComponent(DataCell),

    #[error(
        "The cluster component must be increasingly sorted and not contain \
            any duplicates, got {0:?}"
    )]
    InvalidClusteringComponent(DataCell),

    #[error(
        "Component '{component}' failed to typecheck: expected {expected:#?} but got {got:#?}"
    )]
    TypeCheck {
        component: ComponentName,
        expected: DataType,
        got: DataType,
    },
}

pub type WriteResult<T> = ::std::result::Result<T, WriteError>;

impl DataStore {
    /// Inserts a [`DataTable`]'s worth of components into the datastore.
    ///
    /// This iteratively inserts all rows from the table on a row-by-row basis.
    /// The entire method fails if any row fails.
    ///
    /// Both the write and read paths transparently benefit from the contiguous memory of the
    /// table's columns: the bigger the tables, the bigger the benefits!
    ///
    /// See [`Self::insert_row`].
    pub fn insert_table(&mut self, table: &DataTable) -> WriteResult<()> {
        for row in table.to_rows() {
            self.insert_row(&row)?;
        }
        Ok(())
    }

    /// Inserts a [`DataRow`]'s worth of components into the datastore.
    ///
    /// If the bundle doesn't carry a payload for the cluster key, one will be auto-generated
    /// based on the length of the components in the payload, in the form of an array of
    /// monotonically increasing `u64`s going from `0` to `N-1`.
    pub fn insert_row(&mut self, row: &DataRow) -> WriteResult<()> {
        // TODO(cmc): kind & insert_id need to somehow propagate through the span system.
        self.insert_id += 1;

        if row.num_cells() == 0 {
            return Ok(());
        }

        crate::profile_function!();

        // Update type registry and do typechecking if enabled
        if self.config.enable_typecheck {
            for cell in row.cells().iter() {
                use std::collections::hash_map::Entry;
                match self.type_registry.entry(cell.component_name()) {
                    Entry::Occupied(entry) => {
                        if entry.get() != cell.datatype() {
                            return Err(WriteError::TypeCheck {
                                component: cell.component_name(),
                                expected: entry.get().clone(),
                                got: cell.datatype().clone(),
                            });
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(cell.datatype().clone());
                    }
                }
            }
        } else {
            for cell in row.cells().iter() {
                self.type_registry
                    .insert(cell.component_name(), cell.datatype().clone());
            }
        }

        let DataRow {
            row_id,
            timepoint,
            entity_path: ent_path,
            num_instances,
            cells,
        } = row;

        let ent_path_hash = ent_path.hash();
        let num_instances = *num_instances;

        trace!(
            kind = "insert",
            id = self.insert_id,
            cluster_key = %self.cluster_key,
            timelines = ?timepoint.iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            entity = %ent_path,
            components = ?cells.iter().map(|cell| cell.component_name()).collect_vec(),
            "insertion started..."
        );

        let cluster_cell_pos = cells
            .iter()
            .find_position(|cell| cell.component_name() == self.cluster_key)
            .map(|(pos, _)| pos);

        let generated_cluster_cell = if let Some(cluster_cell_pos) = cluster_cell_pos {
            // We found a column with a name matching the cluster key's, let's make sure it's
            // valid (dense, sorted, no duplicates) and use that if so.

            let cluster_cell = &cells[cluster_cell_pos];

            // Clustering component must be dense.
            if !cluster_cell.is_dense() {
                return Err(WriteError::SparseClusteringComponent(cluster_cell.clone()));
            }
            // Clustering component must be sorted and not contain any duplicates.
            if !cluster_cell.is_sorted_and_unique()? {
                return Err(WriteError::InvalidClusteringComponent(cluster_cell.clone()));
            }

            None
        } else {
            // The caller has not specified any cluster component, and so we'll have to generate
            // one... unless we've already generated one of this exact length in the past,
            // in which case we can simply re-use that cell.

            Some(self.generate_cluster_cell(num_instances))
        };

        let insert_id = self.config.store_insert_ids.then_some(self.insert_id);

        if timepoint.is_timeless() {
            let index = self
                .timeless_tables
                .entry(ent_path_hash)
                .or_insert_with(|| PersistentIndexedTable::new(self.cluster_key, ent_path.clone()));

            index.insert_row(insert_id, generated_cluster_cell, row);
        } else {
            for (timeline, time) in timepoint.iter() {
                let ent_path = ent_path.clone(); // shallow
                let index = self
                    .tables
                    .entry((*timeline, ent_path_hash))
                    .or_insert_with(|| IndexedTable::new(self.cluster_key, *timeline, ent_path));

                index.insert_row(
                    &self.config,
                    insert_id,
                    *time,
                    generated_cluster_cell.clone(), /* shallow */
                    row,
                );
            }
        }

        self.metadata_registry.upsert(*row_id, timepoint.clone());

        Ok(())
    }

    /// Wipes all timeless data.
    ///
    /// Mostly useful for testing/debugging purposes.
    pub fn wipe_timeless_data(&mut self) {
        self.timeless_tables = Default::default();
    }

    /// Auto-generates an appropriate cluster cell for the specified number of instances and
    /// transparently handles caching.
    // TODO(#1777): shared slices for auto generated keys
    fn generate_cluster_cell(&mut self, num_instances: u32) -> DataCell {
        crate::profile_function!();

        if let Some(cell) = self.cluster_cell_cache.get(&num_instances) {
            // Cache hit!

            cell.clone() // shallow
        } else {
            // Cache miss! Craft a new instance keys from the ground up.

            // TODO(#1712): That's exactly how one should create a cell of instance keys...
            // but it turns out that running `TryIntoArrow` on a primitive type is orders of
            // magnitude slower than manually creating the equivalent primitive array for some
            // reason...
            // let cell = DataCell::from_component::<InstanceKey>(0..len as u64);

            // ...so we create it manually instead.
            use re_log_types::Component as _;
            let values =
                arrow2::array::UInt64Array::from_vec((0..num_instances as u64).collect_vec())
                    .boxed();
            let mut cell = DataCell::from_arrow(InstanceKey::name(), values);
            cell.compute_size_bytes();

            self.cluster_cell_cache
                .insert(num_instances, cell.clone() /* shallow */);

            cell
        }
    }
}

impl MetadataRegistry<TimePoint> {
    fn upsert(&mut self, row_id: RowId, timepoint: TimePoint) {
        let mut added_size_bytes = 0;

        // This is valuable information even for a timeless timepoint!
        match self.entry(row_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                // NOTE: In a map, thus on the heap!
                added_size_bytes += row_id.total_size_bytes();
                added_size_bytes += timepoint.total_size_bytes();
                entry.insert(timepoint);
            }
            // NOTE: When saving and loading data from disk, it's very possible that we try to
            // insert data for a single `RowId` in multiple calls (buckets are per-timeline, so a
            // single `RowId` can get spread across multiple buckets)!
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                for (timeline, time) in timepoint {
                    if let Some(old_time) = entry.insert(timeline, time) {
                        if old_time != time {
                            re_log::error!(%row_id, ?timeline, old_time = ?old_time, new_time = ?time, "detected re-used `RowId/Timeline` pair, this is illegal and will lead to undefined behavior in the datastore");
                            debug_assert!(false, "detected re-used `RowId/Timeline`");
                        }
                    } else {
                        // NOTE: In a map, thus on the heap!
                        added_size_bytes += timeline.total_size_bytes();
                        added_size_bytes += time.as_i64().total_size_bytes();
                    }
                }
            }
        }

        self.heap_size_bytes += added_size_bytes;
    }
}

// --- Temporal ---

impl IndexedTable {
    pub fn insert_row(
        &mut self,
        config: &DataStoreConfig,
        insert_id: Option<u64>,
        time: TimeInt,
        generated_cluster_cell: Option<DataCell>,
        row: &DataRow,
    ) {
        crate::profile_function!();

        let components: IntSet<_> = row.component_names().collect();

        // borrowck workaround
        let timeline = self.timeline;
        let ent_path = self.ent_path.clone(); // shallow

        let (_, bucket) = self.find_bucket_mut(time);

        let len = bucket.num_rows();
        let len_overflow = len > config.indexed_bucket_num_rows;

        if len_overflow {
            let bucket_size_before = bucket.total_size_bytes();
            if let Some((min, second_half)) = bucket.split() {
                trace!(
                    kind = "insert",
                    timeline = %timeline.name(),
                    time = timeline.typ().format(time),
                    entity = %ent_path,
                    len_limit = config.indexed_bucket_num_rows,
                    len, len_overflow,
                    new_time_bound = timeline.typ().format(min),
                    "splitting off indexed bucket following overflow"
                );

                self.buckets_size_bytes +=
                    bucket.total_size_bytes() + second_half.total_size_bytes();
                self.buckets_size_bytes -= bucket_size_before;
                self.buckets.insert(min, second_half);

                return self.insert_row(config, insert_id, time, generated_cluster_cell, row);
            }

            // We couldn't split the bucket, either because it's already too small, or because it
            // contains a unique timepoint value that's repeated multiple times.
            //
            // * If the bucket is that small, then there really is no better thing to do than
            //   letting it grow some more by appending to it.
            //
            // * If the timepoint we're trying to insert is smaller or equal to the current upper
            //   bound of the bucket, then at this point we have no choice but to insert it here
            //   (by definition, it is impossible that any previous bucket in the chain covers a
            //   time range that includes this timepoint: buckets are non-overlapping!).
            //
            // * Otherwise, if the timepoint we're trying to insert is greater than the upper bound
            //   of the current bucket, then it means that there currently exist no bucket that
            //   covers a time range which includes this timepoint (if such a bucket existed, then
            //   we would have stumbled upon it before ever finding the current one!).
            //   This gives us an opportunity to create a new bucket that starts at the upper
            //   bound of the current one _excluded_ and that ranges all the way up to the
            //   timepoint that we're inserting.
            //   Not only is this a great opportunity to naturally split things up, it's actually
            //   mandatory to avoid a nasty edge case where one keeps inserting into a full,
            //   unsplittable bucket and indefinitely creates new single-entry buckets, leading
            //   to the worst-possible case of fragmentation.

            let (bucket_upper_bound, bucket_len) = {
                let guard = bucket.inner.read();
                (guard.col_time.last().copied(), guard.col_time.len())
            };

            if let Some(upper_bound) = bucket_upper_bound {
                if bucket_len > 2 && time.as_i64() > upper_bound {
                    let new_time_bound = upper_bound + 1;
                    debug!(
                        kind = "insert",
                        timeline = %timeline.name(),
                        time = timeline.typ().format(time),
                        entity = %ent_path,
                        len_limit = config.indexed_bucket_num_rows,
                        len, len_overflow,
                        new_time_bound = timeline.typ().format(new_time_bound.into()),
                        "creating brand new indexed bucket following overflow"
                    );

                    let (inner, inner_size_bytes) = {
                        let mut inner = IndexedBucketInner {
                            time_range: TimeRange::new(time, time),
                            ..Default::default()
                        };
                        let size_bytes = inner.compute_size_bytes();
                        (inner, size_bytes)
                    };
                    self.buckets.insert(
                        (new_time_bound).into(),
                        IndexedBucket {
                            timeline,
                            cluster_key: self.cluster_key,
                            inner: RwLock::new(inner),
                        },
                    );

                    self.buckets_size_bytes += inner_size_bytes;
                    return self.insert_row(config, insert_id, time, generated_cluster_cell, row);
                }
            }

            debug!(
                kind = "insert",
                timeline = %timeline.name(),
                time = timeline.typ().format(time),
                entity = %ent_path,
                len_limit = config.indexed_bucket_num_rows,
                len, len_overflow,
                "couldn't split indexed bucket, proceeding to ignore limits"
            );
        }

        trace!(
            kind = "insert",
            timeline = %timeline.name(),
            time = timeline.typ().format(time),
            entity = %ent_path,
            ?components,
            "inserted into indexed tables"
        );

        self.buckets_size_bytes +=
            bucket.insert_row(insert_id, time, generated_cluster_cell, row, &components);
        self.buckets_num_rows += 1;

        // Insert components last, only if bucket-insert succeeded.
        self.all_components.extend(components);
    }
}

impl IndexedBucket {
    /// Returns the size in bytes of the inserted arrow data.
    fn insert_row(
        &mut self,
        insert_id: Option<u64>,
        time: TimeInt,
        generated_cluster_cell: Option<DataCell>,
        row: &DataRow,
        components: &IntSet<ComponentName>,
    ) -> u64 {
        crate::profile_function!();

        let mut size_bytes_added = 0u64;
        let num_rows = self.num_rows() as usize;

        let mut inner = self.inner.write();
        let IndexedBucketInner {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            size_bytes,
        } = &mut *inner;

        // append time to primary column and update time range appropriately
        col_time.push(time.as_i64());
        *time_range = TimeRange::new(time_range.min.min(time), time_range.max.max(time));
        size_bytes_added += time.as_i64().total_size_bytes();

        // update all control columns
        if let Some(insert_id) = insert_id {
            col_insert_id.push(insert_id);
            size_bytes_added += insert_id.total_size_bytes();
        }
        col_row_id.push(row.row_id());
        size_bytes_added += row.row_id().total_size_bytes();
        col_num_instances.push(row.num_instances());
        size_bytes_added += row.num_instances().total_size_bytes();

        // insert auto-generated cluster cell if present
        if let Some(cluster_cell) = generated_cluster_cell {
            let component = cluster_cell.component_name();
            let column = columns.entry(component).or_insert_with(|| {
                let column = DataCellColumn::empty(num_rows);
                size_bytes_added += component.total_size_bytes();
                size_bytes_added += column.total_size_bytes();
                column
            });
            size_bytes_added += cluster_cell.total_size_bytes();
            column.0.push(Some(cluster_cell));
        }

        // append components to their respective columns (2-way merge)

        // 2-way merge, step 1: left-to-right
        for cell in row.cells().iter() {
            let component = cell.component_name();
            let column = columns.entry(component).or_insert_with(|| {
                let column = DataCellColumn::empty(col_time.len().saturating_sub(1));
                size_bytes_added += component.total_size_bytes();
                size_bytes_added += column.total_size_bytes();
                column
            });
            size_bytes_added += cell.total_size_bytes();
            column.0.push(Some(cell.clone() /* shallow */));
        }

        // 2-way merge, step 2: right-to-left
        //
        // fill unimpacted columns with null values
        for (component, column) in &mut *columns {
            // The cluster key always gets added one way or another, don't try to force fill it!
            if *component == self.cluster_key {
                continue;
            }

            if !components.contains(component) {
                let none_cell: Option<DataCell> = None;
                size_bytes_added += none_cell.total_size_bytes();
                column.0.push(none_cell);
            }
        }

        // TODO(#433): re_datastore: properly handle already sorted data during insertion
        *is_sorted = false;

        *size_bytes += size_bytes_added;

        #[cfg(debug_assertions)]
        {
            drop(inner);
            self.sanity_check().unwrap();
        }

        size_bytes_added
    }

    /// Splits the bucket into two, potentially uneven parts.
    ///
    /// On success..:
    /// - the first part is split in place (i.e. modifies `self`),
    /// - the second part is returned as a new bucket,
    /// - and the minimal bound of that new bucket is returned as a `TimeInt`, for indexing.
    ///
    /// Returns `None` on failure, i.e. if the bucket cannot be split any further, which can
    /// happen either because the bucket is too small to begin with, or because it only contains
    /// a single timepoint.
    ///
    /// # Unsplittable buckets
    ///
    /// The datastore and query path operate under the general assumption that _all of the data_
    /// for a given timepoint will reside in _one and only one_ bucket.
    /// This function makes sure to uphold that restriction, which sometimes means splitting the
    /// bucket into two uneven parts, or even not splitting it at all.
    ///
    /// Run the following command to display a visualization of the store's internal
    /// datastructures and better understand how everything fits together:
    /// ```text
    /// cargo test -p re_arrow_store -- --nocapture datastore_internal_repr
    /// ```
    //
    // TODO(#1524): inline visualization once it's back to a manageable state
    fn split(&self) -> Option<(TimeInt, Self)> {
        let Self {
            timeline,
            cluster_key: _,
            inner,
        } = self;

        let mut inner1 = inner.write();
        inner1.sort();

        let IndexedBucketInner {
            is_sorted: _,
            time_range: time_range1,
            col_time: col_time1,
            col_insert_id: col_insert_id1,
            col_row_id: col_row_id1,
            col_num_instances: col_num_instances1,
            columns: columns1,
            size_bytes: _, // NOTE: recomputed below
        } = &mut *inner1;

        if col_time1.len() < 2 {
            return None; // early exit: can't split the unsplittable
        }

        if col_time1.first() == col_time1.last() {
            // The entire bucket contains only one timepoint, thus it's impossible to find
            // a split index to begin with.
            return None;
        }

        crate::profile_function!();

        let timeline = *timeline;

        // Used in debug builds to assert that we've left everything in a sane state.
        let _num_rows = col_time1.len();

        fn split_off_column<T: Copy, const N: usize>(
            column: &mut SmallVec<[T; N]>,
            split_idx: usize,
        ) -> SmallVec<[T; N]> {
            if split_idx >= column.len() {
                return SmallVec::default();
            }

            let second_half = SmallVec::from_slice(&column[split_idx..]);
            column.truncate(split_idx);
            second_half
        }

        let (min2, bucket2) = {
            let split_idx = find_split_index(col_time1).expect("must be splittable at this point");

            let (time_range2, col_time2, col_insert_id2, col_row_id2, col_num_instances2) = {
                crate::profile_scope!("control");
                (
                    // this updates `time_range1` in-place!
                    split_time_range_off(split_idx, col_time1, time_range1),
                    // this updates `col_time1` in-place!
                    split_off_column(col_time1, split_idx),
                    // this updates `col_insert_id1` in-place!
                    split_off_column(col_insert_id1, split_idx),
                    // this updates `col_row_id1` in-place!
                    split_off_column(col_row_id1, split_idx),
                    // this updates `col_num_instances1` in-place!
                    split_off_column(col_num_instances1, split_idx),
                )
            };

            // this updates `columns1` in-place!
            let columns2: IntMap<_, _> = {
                crate::profile_scope!("data");
                columns1
                    .iter_mut()
                    .map(|(name, column1)| {
                        if split_idx >= column1.len() {
                            return (*name, DataCellColumn(SmallVec::default()));
                        }

                        // this updates `column1` in-place!
                        let column2 = DataCellColumn({
                            let second_half = SmallVec::from(&column1.0[split_idx..]);
                            column1.0.truncate(split_idx);
                            second_half
                        });
                        (*name, column2)
                    })
                    .collect()
            };

            let inner2 = {
                let mut inner2 = IndexedBucketInner {
                    is_sorted: true,
                    time_range: time_range2,
                    col_time: col_time2,
                    col_insert_id: col_insert_id2,
                    col_row_id: col_row_id2,
                    col_num_instances: col_num_instances2,
                    columns: columns2,
                    size_bytes: 0, // NOTE: computed below
                };
                inner2.compute_size_bytes();
                inner2
            };
            let bucket2 = Self {
                timeline,
                cluster_key: self.cluster_key,
                inner: RwLock::new(inner2),
            };

            (time_range2.min, bucket2)
        };

        inner1.compute_size_bytes();

        // sanity checks
        #[cfg(debug_assertions)]
        {
            drop(inner1); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
            bucket2.sanity_check().unwrap();

            let num_rows1 = self.num_rows() as i64;
            let num_rows2 = bucket2.num_rows() as i64;
            debug_assert_eq!(
                _num_rows as i64,
                num_rows1 + num_rows2,
                "expected both buckets to sum up to the length of the original bucket"
            );
        }

        Some((min2, bucket2))
    }
}

/// Finds an optimal split point for the given time index, or `None` if all entries in the index
/// are identical, making it unsplittable.
///
/// The returned index is _exclusive_: `[0, split_idx)` + `[split_idx; len)`.
///
/// # Panics
///
/// This function expects `times` to be sorted!
/// In debug builds, it will panic if that's not the case.
fn find_split_index(times: &[i64]) -> Option<usize> {
    debug_assert!(
        times.windows(2).all(|t| t[0] <= t[1]),
        "time index must be sorted before splitting!"
    );

    if times.first() == times.last() {
        return None; // early exit: unsplittable
    }

    crate::profile_function!();

    // This can never be lesser than 1 as we never split buckets smaller than 2 entries.
    let halfway_idx = times.len() / 2;
    let target = times[halfway_idx];

    // Are we about to split in the middle of a continuous run? Hop backwards to figure it out.
    let split_idx1 = Some(times[..halfway_idx].partition_point(|&t| t < target)).filter(|&i| i > 0);

    // Are we about to split in the middle of a continuous run? Hop forwards to figure it out.
    let split_idx2 = Some(times[halfway_idx..].partition_point(|&t| t <= target))
        .map(|t| t + halfway_idx) // we skipped that many entries!
        .filter(|&t| t < times.len());

    // Are we in the middle of a backwards continuous run? a forwards continuous run? both?
    match (split_idx1, split_idx2) {
        // Unsplittable, which cannot happen as we already early-exit earlier.
        #[cfg(not(debug_assertions))]
        (None, None) => None,
        #[cfg(debug_assertions)]
        (None, None) => unreachable!(),

        // Backwards run, let's use the first split index.
        (Some(split_idx1), None) => Some(split_idx1),

        // Forwards run, let's use the second split index.
        (None, Some(split_idx2)) => Some(split_idx2),

        // The run goes both backwards and forwards from the half point: use the split index
        // that's the closest to halfway.
        (Some(split_idx1), Some(split_idx2)) => {
            if halfway_idx.abs_diff(split_idx1) < halfway_idx.abs_diff(split_idx2) {
                split_idx1
            } else {
                split_idx2
            }
            .into()
        }
    }
}

#[test]
fn test_find_split_index() {
    let test_cases = [
        (vec![1, 1], None),
        //
        (vec![1, 1, 1], None),
        (vec![1, 1, 2], Some(2)),
        (vec![0, 1, 1], Some(1)),
        //
        (vec![1, 1, 1, 1], None),
        (vec![1, 1, 1, 2], Some(3)),
        (vec![0, 1, 1, 1], Some(1)),
        //
        (vec![1, 1, 1, 1, 1], None),
        (vec![1, 1, 1, 1, 2], Some(4)),
        (vec![0, 1, 1, 1, 1], Some(1)),
        (vec![0, 1, 1, 1, 2], Some(1)), // first one wins when equal distances
        (vec![0, 1, 1, 2, 2], Some(3)), // second one is closer
        (vec![0, 0, 1, 2, 2], Some(2)), // first one wins when equal distances
        (vec![0, 0, 2, 2, 2], Some(2)), // second one is closer
        (vec![0, 0, 0, 2, 2], Some(3)), // first one is closer
    ];

    for (times, expected) in test_cases {
        let got = find_split_index(&times);
        assert_eq!(expected, got);
    }
}

/// Given a time index and a desired split index, splits off the given time range in place,
/// and returns a new time range corresponding to the second part.
///
/// The split index is exclusive: everything up to `split_idx` (excluded) will end up in the
/// first split.
///
/// The two resulting time range halves are guaranteed to never overlap.
fn split_time_range_off(
    split_idx: usize,
    times1: &[i64],
    time_range1: &mut TimeRange,
) -> TimeRange {
    let time_range2 = TimeRange::new(times1[split_idx].into(), time_range1.max);

    // This can never fail (underflow or OOB) because we never split buckets smaller than 2
    // entries.
    time_range1.max = times1[split_idx - 1].into();

    debug_assert!(
        time_range1.max.as_i64() < time_range2.min.as_i64(),
        "split resulted in overlapping time ranges: {} <-> {}\n{:#?}",
        time_range1.max.as_i64(),
        time_range2.min.as_i64(),
        (&time_range1, &time_range2),
    );

    time_range2
}

// --- Timeless ---

impl PersistentIndexedTable {
    fn insert_row(
        &mut self,
        insert_id: Option<u64>,
        generated_cluster_cell: Option<DataCell>,
        row: &DataRow,
    ) {
        crate::profile_function!();

        let num_rows = self.num_rows() as usize;

        let Self {
            ent_path: _,
            cluster_key: _,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        } = self;

        let components: IntSet<_> = row.component_names().collect();

        // --- update all control columns ---

        if let Some(insert_id) = insert_id {
            col_insert_id.push(insert_id);
        }
        col_row_id.push(row.row_id());
        col_num_instances.push(row.num_instances());

        // --- append components to their respective columns (2-way merge) ---

        // insert auto-generated cluster cell if present
        if let Some(cluster_cell) = generated_cluster_cell {
            let column = columns
                .entry(cluster_cell.component_name())
                .or_insert_with(|| DataCellColumn::empty(num_rows));
            column.0.push(Some(cluster_cell));
        }

        // 2-way merge, step 1: left-to-right
        for cell in row.cells().iter() {
            let column = columns
                .entry(cell.component_name())
                .or_insert_with(|| DataCellColumn::empty(num_rows));
            column.0.push(Some(cell.clone() /* shallow */));
        }

        // 2-way merge, step 2: right-to-left
        //
        // fill unimpacted secondary indices with null values
        for (component, column) in columns.iter_mut() {
            // The cluster key always gets added one way or another, don't try to force fill it!
            if *component == self.cluster_key {
                continue;
            }

            if !components.contains(component) {
                column.0.push(None);
            }
        }

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();
    }
}
