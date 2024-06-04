use arrow2::datatypes::DataType;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use parking_lot::RwLock;

use re_log::{debug, trace};
use re_log_types::{
    DataCell, DataCellColumn, DataCellError, DataRow, EntityPathHash, ResolvedTimeRange, RowId,
    TimeInt, TimePoint, VecDequeRemovalExt as _,
};
use re_types_core::{ComponentName, ComponentNameSet, SizeBytes as _};

use crate::{
    DataStore, DataStoreConfig, IndexedBucket, IndexedBucketInner, IndexedTable, MetadataRegistry,
    StaticCell, StaticTable, StoreDiff, StoreDiffKind, StoreEvent,
};

// --- Data store ---

#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    #[error("Error with one or more the underlying data cells")]
    DataCell(#[from] DataCellError),

    #[error("The inserted data must contain at least one cell")]
    Empty,

    #[error(
        "Component '{component}' failed to typecheck: expected {expected:#?} but got {got:#?}"
    )]
    TypeCheck {
        component: ComponentName,
        expected: DataType,
        got: DataType,
    },

    #[error("Attempted to re-use already taken RowId:{0}")]
    ReusedRowId(RowId),
}

pub type WriteResult<T> = ::std::result::Result<T, WriteError>;

impl DataStore {
    /// Inserts a [`DataRow`]'s worth of components into the datastore.
    pub fn insert_row(&mut self, row: &DataRow) -> WriteResult<StoreEvent> {
        // TODO(cmc): kind & insert_id need to somehow propagate through the span system.
        self.insert_id += 1;

        if row.num_cells() == 0 {
            return Err(WriteError::Empty);
        }

        let DataRow {
            row_id,
            timepoint,
            entity_path,
            cells,
        } = row;

        self.metadata_registry
            .upsert(*row_id, (timepoint.clone(), entity_path.hash()))?;

        re_tracing::profile_function!();

        // Update type registry.
        // TODO(#1809): not only this should be replaced by a central arrow runtime registry, it should
        // also be implemented as a changelog subscriber.
        for cell in row.cells().iter() {
            self.type_registry
                .insert(cell.component_name(), cell.datatype().clone());
        }

        let entity_path_hash = entity_path.hash();

        trace!(
            kind = "insert",
            id = self.insert_id,
            timelines = ?timepoint.iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format_utc(*time)))
                .collect::<Vec<_>>(),
            %entity_path,
            components = ?cells.iter().map(|cell| cell.component_name()).collect_vec(),
            "insertion started…"
        );

        let insert_id = self.config.store_insert_ids.then_some(self.insert_id);

        let diff = if timepoint.is_static() {
            let static_table = self
                .static_tables
                .entry(entity_path_hash)
                .or_insert_with(|| StaticTable::new(entity_path.clone()));

            let cells = row
                .cells()
                .iter()
                .filter(|cell| {
                    static_table
                        .cells
                        .get(&cell.component_name())
                        // Last-write-wins semantics, where ordering is defined by RowId.
                        .map_or(true, |static_cell| static_cell.row_id < *row_id)
                })
                .collect_vec();

            for cell in &cells {
                static_table.cells.insert(
                    cell.component_name(),
                    StaticCell {
                        insert_id,
                        row_id: *row_id,
                        cell: (*cell).clone(),
                    },
                );
            }

            let mut diff = StoreDiff::addition(*row_id, entity_path.clone());
            diff.with_cells(cells.into_iter().cloned());
            diff
        } else {
            for (timeline, time) in timepoint.iter() {
                let entity_path = entity_path.clone(); // shallow
                let index = self
                    .tables
                    .entry((entity_path_hash, *timeline))
                    .or_insert_with(|| IndexedTable::new(*timeline, entity_path));

                index.insert_row(&self.config, insert_id, *time, row);
            }

            let mut diff = StoreDiff::addition(*row_id, entity_path.clone());
            diff.at_timepoint(timepoint.clone())
                .with_cells(cells.iter().cloned());
            diff
        };

        let event = StoreEvent {
            store_id: self.id.clone(),
            store_generation: self.generation(),
            event_id: self
                .event_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            diff,
        };

        {
            let events = &[event.clone()];

            if cfg!(debug_assertions) {
                let any_event_other_than_addition =
                    events.iter().any(|e| e.kind != StoreDiffKind::Addition);
                assert!(!any_event_other_than_addition);
            }

            Self::on_events(events);
        }

        Ok(event)
    }
}

impl MetadataRegistry<(TimePoint, EntityPathHash)> {
    fn upsert(&mut self, row_id: RowId, data: (TimePoint, EntityPathHash)) -> WriteResult<()> {
        match self.entry(row_id) {
            std::collections::btree_map::Entry::Occupied(_) => Err(WriteError::ReusedRowId(row_id)),
            std::collections::btree_map::Entry::Vacant(entry) => {
                // NOTE: In a map, thus on the heap!
                let added_size_bytes = row_id.total_size_bytes() + data.total_size_bytes();

                // This is valuable information even for a timeless timepoint!
                entry.insert(data);

                self.heap_size_bytes += added_size_bytes;

                Ok(())
            }
        }
    }
}

// --- Temporal ---

impl IndexedTable {
    pub fn insert_row(
        &mut self,
        config: &DataStoreConfig,
        insert_id: Option<u64>,
        time: TimeInt,
        row: &DataRow,
    ) {
        re_tracing::profile_function!();

        let components: ComponentNameSet = row.component_names().collect();

        // borrowck workaround
        let timeline = self.timeline;
        let entity_path = self.entity_path.clone(); // shallow

        let (_, bucket) = self.find_bucket_mut(time);

        let len = bucket.num_rows();
        let len_overflow = len > config.indexed_bucket_num_rows;

        if len_overflow {
            let bucket_size_before = bucket.total_size_bytes();
            if let Some((min, second_half)) = bucket.split() {
                trace!(
                    kind = "insert",
                    timeline = %timeline.name(),
                    time = timeline.typ().format_utc(time),
                    %entity_path,
                    len_limit = config.indexed_bucket_num_rows,
                    len, len_overflow,
                    new_time_bound = timeline.typ().format_utc(min),
                    "splitting off indexed bucket following overflow"
                );

                self.buckets_size_bytes +=
                    bucket.total_size_bytes() + second_half.total_size_bytes();
                self.buckets_size_bytes -= bucket_size_before;
                self.buckets.insert(min, second_half);

                return self.insert_row(config, insert_id, time, row);
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
                (guard.col_time.back().copied(), guard.col_time.len())
            };

            if let Some(upper_bound) = bucket_upper_bound {
                if bucket_len > 2 && time.as_i64() > upper_bound {
                    let new_time_bound = upper_bound + 1;
                    debug!(
                        kind = "insert",
                        timeline = %timeline.name(),
                        time = timeline.typ().format_utc(time),
                        %entity_path,
                        len_limit = config.indexed_bucket_num_rows,
                        len, len_overflow,
                        new_time_bound = timeline.typ().format_utc(TimeInt::new_temporal(new_time_bound)),
                        "creating brand new indexed bucket following overflow"
                    );

                    let (inner, inner_size_bytes) = {
                        let mut inner = IndexedBucketInner {
                            time_range: ResolvedTimeRange::new(time, time),
                            ..Default::default()
                        };
                        let size_bytes = inner.compute_size_bytes();
                        (inner, size_bytes)
                    };
                    self.buckets.insert(
                        TimeInt::new_temporal(new_time_bound),
                        IndexedBucket {
                            timeline,
                            inner: RwLock::new(inner),
                        },
                    );

                    self.buckets_size_bytes += inner_size_bytes;
                    return self.insert_row(config, insert_id, time, row);
                }
            }

            if 0 < config.indexed_bucket_num_rows {
                let bucket_time_range = bucket.inner.read().time_range;

                re_log::debug_once!("Failed to split bucket on timeline {}", timeline.name());

                if 1 < config.indexed_bucket_num_rows
                    && bucket_time_range.min() == bucket_time_range.max()
                {
                    re_log::warn_once!(
                        "Found over {} rows with the same timepoint {:?}={} - perhaps you forgot to update or remove the timeline?",
                        config.indexed_bucket_num_rows,
                        bucket.timeline.name(),
                        bucket.timeline.typ().format_utc(bucket_time_range.min())
                    );
                }
            }
        }

        trace!(
            kind = "insert",
            timeline = %timeline.name(),
            time = timeline.typ().format_utc(time),
            %entity_path,
            ?components,
            "inserted into indexed tables"
        );

        let size_bytes = bucket.insert_row(insert_id, time, row, &components);
        self.buckets_size_bytes += size_bytes;
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
        row: &DataRow,
        components: &ComponentNameSet,
    ) -> u64 {
        re_tracing::profile_function!();

        let mut size_bytes_added = 0u64;
        let _num_rows = self.num_rows() as usize;

        let mut inner = self.inner.write();
        let IndexedBucketInner {
            is_sorted,
            time_range,
            col_time,
            col_insert_id,
            col_row_id,
            max_row_id,
            columns,
            size_bytes,
        } = &mut *inner;

        // append time to primary column and update time range appropriately

        if let (Some(last_time), Some(last_row_id)) = (col_time.back(), col_row_id.back()) {
            // NOTE: Within a single timestamp, we use the Row ID as tie-breaker
            *is_sorted &= (*last_time, *last_row_id) <= (time.as_i64(), row.row_id());
        }

        col_time.push_back(time.as_i64());
        *time_range =
            ResolvedTimeRange::new(time_range.min().min(time), time_range.max().max(time));
        size_bytes_added += time.as_i64().total_size_bytes();

        // update all control columns
        if let Some(insert_id) = insert_id {
            col_insert_id.push_back(insert_id);
            size_bytes_added += insert_id.total_size_bytes();
        }
        col_row_id.push_back(row.row_id());
        *max_row_id = RowId::max(*max_row_id, row.row_id());
        size_bytes_added += row.row_id().total_size_bytes();

        // append components to their respective columns (2-way merge)

        // 2-way merge, step 1: left-to-right
        for cell in row.cells().iter() {
            let component_name = cell.component_name();
            let column = columns.entry(component_name).or_insert_with(|| {
                let column = DataCellColumn::empty(col_time.len().saturating_sub(1));
                size_bytes_added += component_name.total_size_bytes();
                size_bytes_added += column.total_size_bytes();
                column
            });
            size_bytes_added += cell.total_size_bytes();
            column.0.push_back(Some(cell.clone() /* shallow */));
        }

        // 2-way merge, step 2: right-to-left
        //
        // fill unimpacted columns with null values
        for (component_name, column) in &mut *columns {
            if !components.contains(component_name) {
                let none_cell: Option<DataCell> = None;
                size_bytes_added += none_cell.total_size_bytes();
                column.0.push_back(none_cell);
            }
        }

        *size_bytes += size_bytes_added;

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)]
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
    /// cargo test -p re_data_store -- --nocapture datastore_internal_repr
    /// ```
    fn split(&self) -> Option<(TimeInt, Self)> {
        let Self { timeline, inner } = self;

        let mut inner1 = inner.write();

        if inner1.col_time.len() < 2 {
            return None; // early exit: can't split the unsplittable
        }

        if inner1.time_range.abs_length() == 0 {
            // The entire bucket contains only one timepoint, thus it's impossible to find
            // a split index to begin with.
            return None;
        }

        re_tracing::profile_function!();

        inner1.sort();

        let IndexedBucketInner {
            is_sorted: _,
            time_range: time_range1,
            col_time: col_time1,
            col_insert_id: col_insert_id1,
            col_row_id: col_row_id1,
            max_row_id: max_row_id1,
            columns: columns1,
            size_bytes: _, // NOTE: recomputed below
        } = &mut *inner1;

        let timeline = *timeline;

        // Used in debug builds to assert that we've left everything in a sane state.
        let _num_rows = col_time1.len();

        let (min2, bucket2) = {
            col_time1.make_contiguous();
            let (times1, &[]) = col_time1.as_slices() else {
                unreachable!();
            };
            let split_idx = find_split_index(times1).expect("must be splittable at this point");

            let (time_range2, col_time2, col_insert_id2, col_row_id2) = {
                re_tracing::profile_scope!("control");
                // update everything _in place_!
                (
                    split_time_range_off(split_idx, times1, time_range1),
                    col_time1.split_off_or_default(split_idx),
                    col_insert_id1.split_off_or_default(split_idx),
                    col_row_id1.split_off_or_default(split_idx),
                )
            };
            // NOTE: We _have_ to fullscan here: the bucket is sorted by `(Time, RowId)`, there
            // could very well be a greater lurking in a lesser entry.
            *max_row_id1 = col_row_id1.iter().max().copied().unwrap_or(RowId::ZERO);

            // this updates `columns1` in-place!
            let columns2: IntMap<_, _> = {
                re_tracing::profile_scope!("data");
                columns1
                    .iter_mut()
                    .map(|(name, column1)| {
                        if split_idx >= column1.len() {
                            return (*name, DataCellColumn(Default::default()));
                        }

                        // this updates `column1` in-place!
                        let column2 = DataCellColumn(column1.split_off(split_idx));
                        (*name, column2)
                    })
                    .collect()
            };

            let inner2 = {
                // NOTE: We _have_ to fullscan here: the bucket is sorted by `(Time, RowId)`, there
                // could very well be a greater lurking in a lesser entry.
                let max_row_id2 = col_row_id2.iter().max().copied().unwrap_or(RowId::ZERO);
                let mut inner2 = IndexedBucketInner {
                    is_sorted: true,
                    time_range: time_range2,
                    col_time: col_time2,
                    col_insert_id: col_insert_id2,
                    col_row_id: col_row_id2,
                    max_row_id: max_row_id2,
                    columns: columns2,
                    size_bytes: 0, // NOTE: computed below
                };
                inner2.compute_size_bytes();
                inner2
            };
            let bucket2 = Self {
                timeline,
                inner: RwLock::new(inner2),
            };

            (time_range2.min(), bucket2)
        };

        inner1.compute_size_bytes();

        // sanity checks
        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)]
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

    re_tracing::profile_function!();

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
    time_range1: &mut ResolvedTimeRange,
) -> ResolvedTimeRange {
    let time_range2 =
        ResolvedTimeRange::new(TimeInt::new_temporal(times1[split_idx]), time_range1.max());

    // This can never fail (underflow or OOB) because we never split buckets smaller than 2
    // entries.
    time_range1.set_max(times1[split_idx - 1]);

    debug_assert!(
        time_range1.max().as_i64() < time_range2.min().as_i64(),
        "split resulted in overlapping time ranges: {} <-> {}\n{:#?}",
        time_range1.max().as_i64(),
        time_range2.min().as_i64(),
        (&time_range1, &time_range2),
    );

    time_range2
}
