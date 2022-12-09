use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, ensure};
use arrow2::array::{
    new_empty_array, Array, Int64Vec, ListArray, MutableArray, StructArray, UInt64Vec,
};
use arrow2::bitmap::MutableBitmap;
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::{DataType, Schema};
use parking_lot::RwLock;
use polars::prelude::IndexOfSchema;

use re_log::debug;
use re_log_types::external::arrow2_convert::deserialize::arrow_array_deserialize_iterator;
use re_log_types::{
    ComponentNameRef, ObjPath as EntityPath, TimeInt, TimePoint, TimeRange, Timeline,
    ENTITY_PATH_KEY,
};

use crate::store::IndexBucketIndices;
use crate::{
    ComponentBucket, ComponentTable, DataStore, DataStoreConfig, IndexBucket, IndexTable, RowIndex,
};

// --- Data store ---

impl DataStore {
    /// Inserts a payload of Arrow data into the datastore.
    ///
    /// The payload is expected to hold:
    /// - the entity path,
    /// - the targeted timelines & timepoints,
    /// - and all the components data.
    pub fn insert(&mut self, schema: &Schema, msg: &Chunk<Box<dyn Array>>) -> anyhow::Result<()> {
        // TODO(cmc): kind & insert_id need to somehow propagate through the span system.
        self.insert_id += 1;

        let ent_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| EntityPath::from(path.as_str()))?;
        let ent_path_hash = *ent_path.hash();

        let timelines = extract_timelines(schema, msg)?;
        let components = extract_components(schema, msg)?;

        debug!(
            kind = "insert",
            id = self.insert_id,
            timelines = ?timelines
                .iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            entity = %ent_path,
            components = ?components.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "insertion started..."
        );

        // TODO(cmc): sort the "instances" component, and everything else accordingly!

        let mut indices = HashMap::with_capacity(components.len());
        for (name, component) in components {
            let table = self.components.entry(name.to_owned()).or_insert_with(|| {
                ComponentTable::new(name.to_owned(), component.data_type().clone())
            });

            let row_idx = table.insert(&self.config, &timelines, component)?;
            indices.insert(name, row_idx);
        }

        for (timeline, time) in &timelines {
            let ent_path = ent_path.clone(); // shallow
            let index = self
                .indices
                .entry((*timeline, ent_path_hash))
                .or_insert_with(|| IndexTable::new(*timeline, ent_path));
            index.insert(&self.config, *time, &indices)?;
        }

        Ok(())
    }
}

fn extract_timelines(
    schema: &Schema,
    msg: &Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(Timeline, TimeInt)>> {
    let timelines = schema
        .index_of("timelines") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `timelines` field`"))?;

    let mut timepoints_iter = arrow_array_deserialize_iterator::<TimePoint>(timelines.as_ref())?;

    let timepoint = timepoints_iter
        .next()
        .ok_or_else(|| anyhow!("No rows in timelines."))?;

    ensure!(
        timepoints_iter.next().is_none(),
        "Expected a single TimePoint, but found more!"
    );

    Ok(timepoint.into_iter().collect())
}

fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(ComponentNameRef<'data>, &'data dyn Array)>> {
    let components = schema
        .index_of("components") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| (field.name.as_str(), comp.as_ref()))
        .collect())
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
        let size_overflow = bucket.total_size_bytes() >= config.index_bucket_size_bytes;

        let len = bucket.total_rows();
        let len_overflow = len >= config.index_bucket_nb_rows;

        if size_overflow || len_overflow {
            debug!(
                kind = "insert",
                timeline = %timeline.name(),
                time = timeline.typ().format(time),
                entity = %ent_path,
                size_limit = config.component_bucket_size_bytes,
                len_limit = config.component_bucket_nb_rows,
                size, size_overflow,
                len, len_overflow,
                "allocating new index bucket, previous one overflowed"
            );

            if let Some((min, second_half)) = bucket.split() {
                self.buckets.insert(min, second_half);
                return self.insert(config, time, indices);
            }
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
            time_range: _,
            times,
            indices,
        } = &mut *guard;

        // append time to primary index
        times.push(time.as_i64().into());

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
            index.push(Some(*row_idx));
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

    /// Splits the bucket into two, potentially uneven, optimal parts.
    ///
    /// The first part is done in place (i.e. modifies `self`), while the second part is returned
    /// as a new bucket.
    ///
    /// Returns `None` if the bucket cannot be split any further.
    pub fn split(&self) -> Option<(TimeInt, Self)> {
        if self.indices.read().times.len() < 2 {
            return None; // early exit: can't split the unsplittable
        }

        let Self { timeline, indices } = self;

        let mut indices = indices.write();
        indices.sort();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: time_range1,
            times: times1,
            indices: indices1,
        } = &mut *indices;

        let timeline = *timeline;
        // Used down the line to assert that we've left everything in a sane state.
        #[cfg(debug_assertions)]
        let total_rows = times1.len();

        let (min2, bucket2) = if let Some(split_idx) = find_split_index(times1) {
            let time_range2 = split_time_range_off(split_idx, times1, time_range1);
            let times2 = split_primary_index_off(split_idx, times1);
            let indices2: HashMap<_, _> = indices1
                .iter_mut()
                .map(|(name, index1)| {
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
        } else {
            // We couldn't find an optimal split index, so we'll just append to the current bucket,
            // even though this is sub-optimal.
            // The good news is that we are now guaranteed to split the next time an insertion is
            // scheduled for that bucket.
            return None;
        };

        // sanity checks
        #[cfg(debug_assertions)]
        {
            drop(indices); // sanity checking will grab the lock!
            self.sanity_check().unwrap();
            bucket2.sanity_check().unwrap();

            let total_rows1 = self.total_rows() as i64;
            let total_rows2 = bucket2.total_rows() as i64;
            assert!(
                total_rows as i64 == total_rows1 + total_rows2,
                "expected both buckets to sum up to the length of the original bucket: \
                    got bucket={} vs. bucket1+bucket2={}",
                total_rows,
                total_rows1 + total_rows2,
            );
            assert_eq!(total_rows as i64, total_rows1 + total_rows2);
        }

        Some((min2, bucket2))
    }
}

/// Finds a split index that is both optimal _and_ still upholds the guarantee that a specific
/// timepoint won't end up being splitted across multiple buckets.
///
/// The returned index is _exclusive_: `[0, split_idx)` + `[split_idx; len)`.
///
/// # What's the deal with timepoints splitted across multiple buckets?
///
/// The datastore and query path operate under the general assumption that _all of the
/// index data_ for a given timepoint will reside in _one and only one_ bucket.
/// This function makes sure to uphold that restriction.
///
/// Here's an example of an index table configured to have a maximum of 2 rows per bucket: we
/// can see that the 1st and 2nd buckets exceed this maximum in order to uphold the restriction
/// described above:
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
//
// TODO(cmc): replace forwards/backwards walk with forwards/backwards binsearches.
fn find_split_index(times: &Int64Vec) -> Option<usize> {
    debug_assert!(
        times.validity().is_none(),
        "The time index must always be dense, thus it shouldn't even have a validity\
            bitmap attached to it to begin with."
    );
    let times = times.values();

    // This can never be lesser than 1 as we never split buckets smaller than 2 entries.
    let split_idx = times.len() / 2;

    // Are we about to split in the middle of a continuous run?
    // We'll walk backwards to figure it out.
    let split_idx1 = {
        let time = times[split_idx];
        let mut split_idx = split_idx as i64;
        loop {
            if split_idx < 0 {
                break None;
            }
            if times[split_idx as usize] != time {
                break Some(split_idx as usize + 1); // +1 because exclusive
            }
            split_idx -= 1;
        }
    };

    // Are we about to split in the middle of a continuous run?
    // We'll now walk forwards to figure it out.
    let split_idx2 = {
        let time = times[split_idx];
        let mut split_idx = split_idx;
        loop {
            if split_idx >= times.len() {
                break None;
            }
            if times[split_idx] != time {
                break Some(split_idx);
            }
            split_idx += 1;
        }
    };

    // Are we in the middle of a backwards continuous run? a forwards continuous run? both?
    match (split_idx1, split_idx2) {
        (None, None) => None,
        // Backwards run, let's use the first split index.
        (Some(split_idx1), None) => Some(split_idx1),
        // Forwards run, let's use the second split index.
        (None, Some(split_idx2)) => Some(split_idx2),
        // The run goes both backwards and forwards from the half point: use the split index
        // that's the closest to halfway.
        (Some(split_idx1), Some(split_idx2)) => {
            if split_idx.abs_diff(split_idx1) < split_idx.abs_diff(split_idx2) {
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
        "The time index must always be dense, thus it shouldn't even have a validity\
            bitmap attached to it to begin with."
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
    if let Some((validity1, validity2)) = validity1.map(|validity1| {
        let mut validity1 = validity1.into_iter().collect::<Vec<_>>();
        let validity2 = validity1.split_off(split_idx);
        (
            MutableBitmap::from_iter(validity1),
            MutableBitmap::from_iter(validity2),
        )
    }) {
        *index1 = UInt64Vec::from_data(datatype.clone(), data1, Some(validity1));
        UInt64Vec::from_data(datatype, data2, Some(validity2))
    } else {
        *index1 = UInt64Vec::from_data(datatype.clone(), data1, None);
        UInt64Vec::from_data(datatype, data2, None)
    }
}

// --- Components ---

impl ComponentTable {
    fn new(name: String, datatype: DataType) -> Self {
        let name = Arc::new(name);
        ComponentTable {
            name: Arc::clone(&name),
            datatype: datatype.clone(),
            buckets: [ComponentBucket::new(name, datatype, 0)].into(),
        }
    }

    pub fn insert(
        &mut self,
        config: &DataStoreConfig,
        timelines: &[(Timeline, TimeInt)],
        data: &dyn Array,
    ) -> anyhow::Result<RowIndex> {
        // All component tables spawn with an initial bucket at row offset 0, thus this cannot
        // fail.
        let bucket = self.buckets.back().unwrap();

        let size = bucket.total_size_bytes();
        let size_overflow = bucket.total_size_bytes() >= config.component_bucket_size_bytes;

        let len = bucket.total_rows();
        let len_overflow = len >= config.component_bucket_nb_rows;

        if size_overflow || len_overflow {
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

            let row_offset = bucket.row_offset + len;
            self.buckets.push_back(ComponentBucket::new(
                Arc::clone(&self.name),
                self.datatype.clone(),
                row_offset,
            ));
        }

        // Two possible cases:
        // - If the table has not just underwent an overflow, then this is panic-safe for the
        //   same reason as above: all component tables spawn with an initial bucket at row
        //   offset 0, thus this cannot fail.
        // - If the table has just overflowed, then we've just pushed a bucket to the dequeue.
        let row_idx = self.buckets.back_mut().unwrap().insert(timelines, data)?;

        debug!(
            kind = "insert",
            timelines = ?timelines
                .iter()
                .map(|(timeline, time)| (timeline.name(), timeline.typ().format(*time)))
                .collect::<Vec<_>>(),
            component = self.name.as_str(),
            row_idx,
            "inserted into component table"
        );

        Ok(row_idx)
    }
}

impl ComponentBucket {
    pub fn new(name: Arc<String>, datatype: DataType, row_offset: RowIndex) -> Self {
        // If this is the first bucket of this table, we need to insert an empty list at
        // row index #0!
        let data = if row_offset == 0 {
            let inner_datatype = match &datatype {
                DataType::List(field) => field.data_type().clone(),
                #[allow(clippy::todo)]
                _ => todo!("throw an error here, this should always be a list"),
            };

            let empty = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(inner_datatype.clone()),
                Buffer::from(vec![0, 0i32]),
                new_empty_array(inner_datatype),
                None,
            );

            // TODO(#451): throw error (or just implement mutable array)
            concatenate(&[&*new_empty_array(datatype), &*empty.boxed()]).unwrap()
        } else {
            new_empty_array(datatype)
        };

        Self {
            name,
            row_offset,
            time_ranges: Default::default(),
            data,
        }
    }

    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TimeInt)],
        data: &dyn Array,
    ) -> anyhow::Result<RowIndex> {
        for (timeline, time) in timelines {
            // TODO(#451): prob should own it at this point
            let time = *time;
            self.time_ranges
                .entry(*timeline)
                .and_modify(|range| {
                    *range = TimeRange::new(range.min.min(time), range.max.max(time));
                })
                .or_insert_with(|| TimeRange::new(time, time));
        }

        // TODO(cmc): replace with an actual mutable array!
        self.data = concatenate(&[&*self.data, data])?;

        Ok(self.row_offset + self.data.len() as u64 - 1)
    }
}
