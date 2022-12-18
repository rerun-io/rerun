use std::collections::HashMap;
use std::sync::Arc;

use anyhow::ensure;
use arrow2::array::{
    new_empty_array, Array, Int64Vec, ListArray, MutableArray, UInt64Array, UInt64Vec,
};
use arrow2::bitmap::MutableBitmap;
use arrow2::buffer::Buffer;
use arrow2::datatypes::DataType;
use itertools::Itertools as _;
use parking_lot::RwLock;

use re_log::debug;
use re_log_types::msg_bundle::{ComponentBundle, MsgBundle};
use re_log_types::{
    ComponentNameRef, ObjPath as EntityPath, TimeInt, TimePoint, TimeRange, Timeline,
};

use crate::store::IndexBucketIndices;
use crate::{
    ComponentBucket, ComponentTable, DataStore, DataStoreConfig, IndexBucket, IndexTable, RowIndex,
};

// --- Data store ---

impl DataStore {
    /// Inserts a [`MsgBundle`]'s worth of components into the datastore.
    ///
    /// * All components across the bundle must share the same number of rows.
    /// * All components within a single row must share the same number of instances.
    pub fn insert(&mut self, bundle: &MsgBundle) -> anyhow::Result<()> {
        // TODO(cmc): kind & insert_id need to somehow propagate through the span system.
        self.insert_id += 1;

        let MsgBundle {
            msg_id: _,
            obj_path: ent_path,
            time_point,
            components,
        } = bundle;

        if components.is_empty() {
            return Ok(());
        }

        let ent_path_hash = *ent_path.hash();
        let nb_rows = components[0].value.len();

        // TODO(#527): typed error
        ensure!(
            nb_rows == 1,
            "we currently don't support more than one row per batch, as a `MsgBundle` can only \
                carry a single timepoint for now!"
        );

        // TODO(#527): typed error
        ensure!(
            components
                .iter()
                .all(|bundle| bundle.value.len() == nb_rows),
            "all components across the bundle must share the same number of rows",
        );

        debug!(
            kind = "insert",
            id = self.insert_id,
            clustering_key = self.clustering_key,
            timelines = ?time_point.iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            entity = %ent_path,
            components = ?components.iter().map(|bundle| &bundle.name).collect::<Vec<_>>(),
            nb_rows,
            "insertion started..."
        );

        let mut row_indices = HashMap::with_capacity(components.len());

        // TODO(#589): support for batched row component insertions
        for row_nr in 0..nb_rows {
            let clustering_comp = get_or_create_clustering_key(components, &self.clustering_key);
            let expected_nb_instances = clustering_comp.len();

            for bundle in components {
                let ComponentBundle { name, value: rows } = bundle;

                // Unwrapping a ListArray is somewhat costly, especially considering we're just
                // gonna rewrap it again in a minute... so we'd rather just slice it to a list of
                // one instead.
                //
                // let rows_single = rows.slice(row_nr, 1);
                //
                // Except it turns out that slicing is _extremely_ costly!
                // So use the fact that `rows` is always of unit-length for now.
                let rows_single = rows;

                let nb_instances = rows_single
                    .as_any()
                    .downcast_ref::<ListArray<i32>>()
                    .unwrap()
                    .value(0)
                    .len();
                // TODO(#527): typed error
                ensure!(
                    expected_nb_instances == nb_instances,
                    "all components in the row must have the same number of instances as the \
                        clustering component",
                );

                let table = self
                    .components
                    .entry((*bundle.name).to_owned())
                    .or_insert_with(|| {
                        let row = rows_single
                            .as_any()
                            .downcast_ref::<ListArray<i32>>()
                            .unwrap()
                            .value(row_nr);
                        ComponentTable::new((*name).clone(), row.data_type())
                    });

                let row_idx = table.push(&self.config, time_point, rows_single.as_ref());
                row_indices.insert(name.as_ref(), row_idx);
            }
        }

        for (timeline, time) in time_point.iter() {
            let ent_path = ent_path.clone(); // shallow
            let index = self
                .indices
                .entry((*timeline, ent_path_hash))
                .or_insert_with(|| IndexTable::new(*timeline, ent_path));
            index.insert(&self.config, *time, &row_indices)?;
        }

        Ok(())
    }
}

// TODO: doc
fn get_or_create_clustering_key(
    components: &[ComponentBundle],
    clustering_key: ComponentNameRef<'_>,
) -> Box<dyn Array> {
    let clustering_comp = components
        .iter()
        .find(|bundle| bundle.name == clustering_key);

    // TODO: debug logs?
    if let Some(clustering_comp) = clustering_comp {
        let first_row = clustering_comp
            .value
            .as_any()
            .downcast_ref::<ListArray<i32>>()
            .unwrap()
            .value(0);
        first_row
    } else {
        let len = components.first().map_or(0, |comp| {
            // TODO(#589): support for batched row component insertions
            let first_row = comp
                .value
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .value(0);
            first_row.len()
        });
        UInt64Array::from_vec((0..len as u64).collect_vec()).boxed()
    }
}

// --- Indices ---

impl IndexTable {
    pub fn new(timeline: Timeline, ent_path: EntityPath) -> Self {
        Self {
            timeline,
            ent_path,
            buckets: [(0.into(), IndexBucket::new(timeline))].into(),
        }
    }

    pub fn insert(
        &mut self,
        config: &DataStoreConfig,
        time: TimeInt,
        indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        // borrowck workaround
        let timeline = self.timeline;
        let ent_path = self.ent_path.clone(); // shallow

        let bucket = self.find_bucket_mut(time.as_i64());

        let size = bucket.total_size_bytes();
        let size_overflow = bucket.total_size_bytes() > config.index_bucket_size_bytes;

        let len = bucket.total_rows();
        let len_overflow = len > config.index_bucket_nb_rows;

        if size_overflow || len_overflow {
            if let Some((min, second_half)) = bucket.split() {
                debug!(
                    kind = "insert",
                    timeline = %timeline.name(),
                    time = timeline.typ().format(time),
                    entity = %ent_path,
                    size_limit = config.component_bucket_size_bytes,
                    len_limit = config.component_bucket_nb_rows,
                    size, size_overflow,
                    len, len_overflow,
                    new_time_bound = timeline.typ().format(min),
                    "splitting off index bucket following overflow"
                );

                self.buckets.insert(min, second_half);
                return self.insert(config, time, indices);
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
            //   bound of the current one _excluded_ and that ranges all the way up to the timepoint
            //   that we're inserting.
            //   Not only is this a great opportunity to naturally split things up, it's actually
            //   mandatory to avoid a nasty edge case where one keeps inserting into a full,
            //   unsplittable bucket and indefinitely creates new single-entry buckets, leading
            //   to the worst-possible case of fragmentation.

            let (bucket_upper_bound, bucket_len) = {
                let guard = bucket.indices.read();
                (guard.times.values().last().copied(), guard.times.len())
            };

            if let Some(upper_bound) = bucket_upper_bound {
                if bucket_len > 2 && time.as_i64() > upper_bound {
                    let new_time_bound = upper_bound + 1;
                    debug!(
                        kind = "insert",
                        timeline = %timeline.name(),
                        time = timeline.typ().format(time),
                        entity = %ent_path,
                        size_limit = config.component_bucket_size_bytes,
                        len_limit = config.component_bucket_nb_rows,
                        size, size_overflow,
                        len, len_overflow,
                        new_time_bound = timeline.typ().format(new_time_bound.into()),
                        "creating brand new index bucket following overflow"
                    );
                    self.buckets.insert(
                        (new_time_bound).into(),
                        IndexBucket {
                            timeline,
                            indices: RwLock::new(IndexBucketIndices {
                                is_sorted: true,
                                time_range: TimeRange::new(time, time),
                                times: Int64Vec::new(),
                                indices: HashMap::default(),
                            }),
                        },
                    );
                    return self.insert(config, time, indices);
                }
            }

            debug!(
                kind = "insert",
                timeline = %timeline.name(),
                time = timeline.typ().format(time),
                entity = %ent_path,
                size_limit = config.component_bucket_size_bytes,
                len_limit = config.component_bucket_nb_rows,
                size, size_overflow,
                len, len_overflow,
                "couldn't split index bucket, proceeding to ignore limits"
            );
        }

        debug!(
            kind = "insert",
            timeline = %timeline.name(),
            time = timeline.typ().format(time),
            entity = %ent_path,
            components = ?indices.iter().collect::<Vec<_>>(),
            "inserted into index table"
        );

        bucket.insert(time, indices)
    }
}

impl IndexBucket {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            indices: RwLock::new(IndexBucketIndices::default()),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn insert(
        &mut self,
        time: TimeInt,
        row_indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        let mut guard = self.indices.write();
        let IndexBucketIndices {
            is_sorted,
            time_range,
            times,
            indices,
        } = &mut *guard;

        // append time to primary index and update time range approriately
        times.push(time.as_i64().into());
        *time_range = TimeRange::new(time_range.min.min(time), time_range.max.max(time));

        // append components to secondary indices (2-way merge)

        // 2-way merge, step1: left-to-right
        //
        // push new row indices to their associated secondary index
        for (name, row_idx) in row_indices {
            let index = indices.entry((*name).to_owned()).or_insert_with(|| {
                let mut index = UInt64Vec::default();
                index.extend_constant(times.len().saturating_sub(1), None);
                index
            });
            index.push(Some(row_idx.as_u64()));
        }

        // 2-way merge, step2: right-to-left
        //
        // fill unimpacted secondary indices with null values
        for (name, index) in &mut *indices {
            if !row_indices.contains_key(name.as_str()) {
                index.push_null();
            }
        }

        // TODO(#433): re_datastore: properly handle already sorted data during insertion
        *is_sorted = false;

        #[cfg(debug_assertions)]
        {
            drop(guard); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
        }

        Ok(())
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
    /// The datastore and query path operate under the general assumption that _all of the
    /// index data_ for a given timepoint will reside in _one and only one_ bucket.
    /// This function makes sure to uphold that restriction, which sometimes means splitting the
    /// bucket into two uneven parts, or even not splitting it at all.
    ///
    /// Here's an example of an index table configured to have a maximum of 2 rows per bucket: one
    /// can see that the 1st and 2nd buckets exceed this maximum in order to uphold the restriction
    /// described above:
    /// ```text
    /// IndexTable {
    ///     timeline: frame_nr
    ///     entity: this/that
    ///     size: 3 buckets for a total of 265 B across 8 total rows
    ///     buckets: [
    ///         IndexBucket {
    ///             index time bound: >= #0
    ///             size: 99 B across 3 rows
    ///             time range: from #41 to #41 (all inclusive)
    ///             data (sorted=true): shape: (3, 4)
    ///             ┌──────┬───────┬───────────┬───────────┐
    ///             │ time ┆ rects ┆ positions ┆ instances │
    ///             │ ---  ┆ ---   ┆ ---       ┆ ---       │
    ///             │ str  ┆ u64   ┆ u64       ┆ u64       │
    ///             ╞══════╪═══════╪═══════════╪═══════════╡
    ///             │ #41  ┆ null  ┆ null      ┆ 1         │
    ///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
    ///             │ #41  ┆ null  ┆ 1         ┆ null      │
    ///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
    ///             │ #41  ┆ 3     ┆ null      ┆ null      │
    ///             └──────┴───────┴───────────┴───────────┘
    ///         }
    ///         IndexBucket {
    ///             index time bound: >= #42
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
    ///             index time bound: >= #43
    ///             size: 67 B across 2 rows
    ///             time range: from #43 to #44 (all inclusive)
    ///             data (sorted=true): shape: (2, 4)
    ///             ┌──────┬───────┬───────────┬───────────┐
    ///             │ time ┆ rects ┆ instances ┆ positions │
    ///             │ ---  ┆ ---   ┆ ---       ┆ ---       │
    ///             │ str  ┆ u64   ┆ u64       ┆ u64       │
    ///             ╞══════╪═══════╪═══════════╪═══════════╡
    ///             │ #43  ┆ 4     ┆ null      ┆ null      │
    ///             ├╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
    ///             │ #44  ┆ null  ┆ null      ┆ 3         │
    ///             └──────┴───────┴───────────┴───────────┘
    ///         }
    ///     ]
    /// }
    /// ```
    pub fn split(&self) -> Option<(TimeInt, Self)> {
        let Self { timeline, indices } = self;

        let mut indices = indices.write();
        indices.sort();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: time_range1,
            times: times1,
            indices: indices1,
        } = &mut *indices;

        if times1.len() < 2 {
            return None; // early exit: can't split the unsplittable
        }

        if times1.values().first() == times1.values().last() {
            // The entire bucket contains only one timepoint, thus it's impossible to find
            // a split index to begin with.
            return None;
        }

        let timeline = *timeline;
        // Used down the line to assert that we've left everything in a sane state.
        let total_rows = times1.len();

        let (min2, bucket2) = {
            let split_idx = find_split_index(times1).expect("must be splittable at this point");

            // this updates `time_range1` in-place!
            let time_range2 = split_time_range_off(split_idx, times1, time_range1);

            // this updates `times1` in-place!
            let times2 = split_primary_index_off(split_idx, times1);

            // this updates `indices1` in-place!
            let indices2: HashMap<_, _> = indices1
                .iter_mut()
                .map(|(name, index1)| {
                    // this updates `index1` in-place!
                    let index2 = split_secondary_index_off(split_idx, index1);
                    ((*name).clone(), index2)
                })
                .collect();
            (
                time_range2.min,
                Self {
                    timeline,
                    indices: RwLock::new(IndexBucketIndices {
                        is_sorted: true,
                        time_range: time_range2,
                        times: times2,
                        indices: indices2,
                    }),
                },
            )
        };

        // sanity checks
        if cfg!(debug_assertions) {
            drop(indices); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
            bucket2.sanity_check().unwrap();

            let total_rows1 = self.total_rows() as i64;
            let total_rows2 = bucket2.total_rows() as i64;
            debug_assert!(
                total_rows as i64 == total_rows1 + total_rows2,
                "expected both buckets to sum up to the length of the original bucket: \
                    got bucket={} vs. bucket1+bucket2={}",
                total_rows,
                total_rows1 + total_rows2,
            );
            debug_assert_eq!(total_rows as i64, total_rows1 + total_rows2);
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
fn find_split_index(times: &Int64Vec) -> Option<usize> {
    debug_assert!(
        times.validity().is_none(),
        "The time index must always be dense."
    );
    debug_assert!(
        times.values().windows(2).all(|t| t[0] <= t[1]),
        "time index must be sorted before splitting!"
    );

    let times = times.values();
    if times.first() == times.last() {
        return None; // early exit: unsplittable
    }

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
        let times = Int64Vec::from_vec(times);
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
    times1: &Int64Vec,
    time_range1: &mut TimeRange,
) -> TimeRange {
    let times1 = times1.values();

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

/// Given a primary time index and a desired split index, splits off the time index in place,
/// and returns a new time index corresponding to the second part.
///
/// The split index is exclusive: everything up to `split_idx` (excluded) will end up in the
/// first split.
fn split_primary_index_off(split_idx: usize, times1: &mut Int64Vec) -> Int64Vec {
    debug_assert!(
        times1.validity().is_none(),
        "The time index must always be dense.",
    );

    let total_rows = times1.len();

    let (datatype, mut data1, _) = std::mem::take(times1).into_data();
    let data2 = data1.split_off(split_idx);
    let times2 = Int64Vec::from_data(datatype.clone(), data2, None);

    *times1 = Int64Vec::from_data(datatype, data1, None);

    debug_assert!(
        total_rows == times1.len() + times2.len(),
        "expected both halves to sum up to the length of the original time index: \
            got times={} vs. times1+times2={}",
        total_rows,
        times1.len() + times2.len(),
    );

    times2
}

/// Given a secondary index of any kind and a desired split index, splits off the index
/// in place, and returns a new index of the same kind that corresponds to the second part.
///
/// The split index is exclusive: everything up to `split_idx` (excluded) will end up in the
/// first split.
fn split_secondary_index_off(split_idx: usize, index1: &mut UInt64Vec) -> UInt64Vec {
    let (datatype, mut data1, validity1) = std::mem::take(index1).into_data();
    let data2 = data1.split_off(split_idx);

    let validities = validity1.map(|validity1| {
        let mut validity1 = validity1.into_iter().collect::<Vec<_>>();
        let validity2 = validity1.split_off(split_idx);
        (
            MutableBitmap::from_iter(validity1),
            MutableBitmap::from_iter(validity2),
        )
    });

    // We can only end up with either no validity bitmap (because the original index didn't have
    // one), or with two new bitmaps (because we've split the original in two), nothing in
    // between.
    if let Some((validity1, validity2)) = validities {
        *index1 = UInt64Vec::from_data(datatype.clone(), data1, Some(validity1));
        UInt64Vec::from_data(datatype, data2, Some(validity2))
    } else {
        *index1 = UInt64Vec::from_data(datatype.clone(), data1, None);
        UInt64Vec::from_data(datatype, data2, None)
    }
}

// --- Components ---

impl ComponentTable {
    /// Creates a new component table for the specified component `datatype`.
    ///
    /// `datatype` must be the type of the component itself, devoid of any wrapping layers
    /// (i.e. _not_ a `ListArray<...>`!).
    fn new(name: String, datatype: &DataType) -> Self {
        let name = Arc::new(name);
        ComponentTable {
            name: Arc::clone(&name),
            datatype: datatype.clone(),
            buckets: [ComponentBucket::new(
                name,
                datatype,
                RowIndex::from_u64(0u64),
            )]
            .into(),
        }
    }

    /// Finds the appropriate bucket in this component table and pushes `rows_single` at the
    /// end of it, returning the _global_ `RowIndex` for this new row.
    ///
    /// `rows_single` must be a list of list of components,
    /// i.e. `ListArray<ListArray<StructArray>>`:
    /// - the first list layer corresponds to the rows and _must be unit-lengthed_,
    /// - the second list layer corresponds to the different instances within the row,
    /// - the struct layer is the component itself.
    /// E.g.:
    /// ```ignore
    /// [[{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}]]
    /// ```
    //
    // TODO(#589): support for batched row component insertions
    pub fn push(
        &mut self,
        config: &DataStoreConfig,
        time_point: &TimePoint,
        rows_single: &dyn Array,
    ) -> RowIndex {
        debug_assert!(
            rows_single
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .value(0)
                .data_type()
                == &self.datatype,
            "trying to insert data of the wrong datatype in a component table",
        );

        // All component tables spawn with an initial bucket at row offset 0, thus this cannot
        // fail.
        let active_bucket = self.buckets.back_mut().unwrap();

        let size = active_bucket.total_size_bytes();
        let size_overflow = active_bucket.total_size_bytes() > config.component_bucket_size_bytes;

        let len = active_bucket.total_rows();
        let len_overflow = len > config.component_bucket_nb_rows;

        if len_overflow {
            debug!(
                kind = "insert",
                component = self.name.as_str(),
                size_limit = config.component_bucket_size_bytes,
                len_limit = config.component_bucket_nb_rows,
                size,
                size_overflow,
                len,
                len_overflow,
                "allocating new component bucket, previous one overflowed"
            );

            // Retire currently active bucket.
            active_bucket.retire();

            let row_offset = active_bucket.row_offset.as_u64() + len;
            self.buckets.push_back(ComponentBucket::new(
                Arc::clone(&self.name),
                &self.datatype,
                RowIndex::from_u64(row_offset),
            ));
        }

        // Two possible cases:
        // - If the table has not just underwent an overflow, then this is panic-safe for the
        //   same reason as above: all component tables spawn with an initial bucket at row
        //   offset 0, thus this cannot fail.
        // - If the table has just overflowed, then we've just pushed a bucket to the dequeue.
        let active_bucket = self.buckets.back_mut().unwrap();
        let row_idx = RowIndex::from_u64(
            active_bucket.push(time_point, rows_single) + active_bucket.row_offset.as_u64(),
        );

        debug!(
            kind = "insert",
            timelines = ?time_point.into_iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            component = self.name.as_str(),
            %row_idx,
            "pushed into component table"
        );

        row_idx
    }
}

impl ComponentBucket {
    /// Creates a new component bucket for the specified component `datatype`.
    ///
    /// `datatype` must be the type of the component itself, devoid of any wrapping layers
    /// (i.e. _not_ a `ListArray<...>`!).
    pub fn new(name: Arc<String>, datatype: &DataType, row_offset: RowIndex) -> Self {
        // If this is the first bucket of this table, we need to insert an empty list at
        // row index #0!
        let chunks = if row_offset.as_u64() == 0 {
            let empty = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(datatype.clone()),
                Buffer::from(vec![0, 0i32]),
                new_empty_array(datatype.clone()),
                None,
            );
            vec![empty.boxed()]
        } else {
            vec![]
        };

        let total_rows = chunks.iter().map(|values| values.len() as u64).sum();
        let total_size_bytes = chunks
            .iter()
            .map(|values| arrow2::compute::aggregate::estimated_bytes_size(&**values) as u64)
            .sum();

        Self {
            name,
            row_offset,
            retired: false,
            time_ranges: Default::default(),
            chunks,
            total_rows,
            total_size_bytes,
        }
    }

    /// Pushes `rows_single` to the end of the bucket, returning the _local_ index of the
    /// freshly added row.
    ///
    /// `rows_single` must be a list of list of components,
    /// i.e. `ListArray<ListArray<StructArray>>`:
    /// - the first list layer corresponds to the rows and _must be unit-lengthed_,
    /// - the second list layer corresponds to the different instances within the row,
    /// - the struct layer is the component itself.
    /// E.g.:
    /// ```ignore
    /// [[{x: 8.687487, y: 1.9590926}, {x: 2.0559108, y: 0.1494348}, {x: 7.09219, y: 0.9616637}]]
    /// ```
    pub fn push(&mut self, time_point: &TimePoint, rows_single: &dyn Array) -> u64 {
        // Keep track of all affected time ranges, for garbage collection purposes.
        for (timeline, &time) in time_point {
            self.time_ranges
                .entry(*timeline)
                .and_modify(|range| {
                    *range = TimeRange::new(range.min.min(time), range.max.max(time));
                })
                .or_insert_with(|| TimeRange::new(time, time));
        }

        self.total_rows += 1;
        // Warning: this is surprisingly costly!
        self.total_size_bytes +=
            arrow2::compute::aggregate::estimated_bytes_size(rows_single) as u64;

        // TODO(#589): support for non-unit-length chunks
        self.chunks.push(rows_single.to_boxed()); // shallow

        self.chunks.len() as u64 - 1
    }

    /// Retires the bucket as a new one is about to take its place.
    ///
    /// This is a good opportunity to run compaction and other maintenance related tasks.
    pub fn retire(&mut self) {
        debug_assert!(
            !self.retired,
            "retiring an already retired bucket, something is likely wrong"
        );

        // Chunk compaction
        // Compacts the bucket by concatenating all chunks of data into a single one.
        {
            use arrow2::compute::concatenate::concatenate;

            let chunks = self.chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            // Only two reasons this can ever fail:
            //
            // * `chunks` is empty:
            // This can never happen, buckets always spawn with an initial chunk.
            //
            // * the various chunks contain data with different datatypes:
            // This can never happen as that would first panic during insertion.
            let values = concatenate(&chunks).unwrap();

            // Recompute the size as we've just discarded a bunch of list headers.
            self.total_size_bytes =
                arrow2::compute::aggregate::estimated_bytes_size(&*values) as u64;

            self.chunks = vec![values];
        }

        self.retired = true;
    }
}
