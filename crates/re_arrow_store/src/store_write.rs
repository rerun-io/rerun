use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure};
use arrow2::array::{
    new_empty_array, Array, Int64Array, Int64Vec, ListArray, MutableArray, StructArray, UInt64Vec,
};
use arrow2::bitmap::MutableBitmap;
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::{DataType, Schema};
use polars::prelude::IndexOfSchema;

use re_log::debug;
use re_log_types::arrow::{ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME};
use re_log_types::{
    ComponentNameRef, ObjPath as EntityPath, TimeInt, TimeRange, TimeType, Timeline,
};

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
        let ent_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| EntityPath::from(path.as_str()))?;
        let ent_path_hash = *ent_path.hash();

        let timelines = extract_timelines(schema, msg)?;
        let components = extract_components(schema, msg)?;

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

    let timelines = timelines
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect top-level `timelines` to be a `StructArray`"))?;

    // implicit Vec<Result> to Result<Vec> collection
    let timelines: Result<Vec<_>, _> = timelines
        .fields()
        .iter()
        .zip(timelines.values())
        .map(
            |(timeline, time)| match timeline.metadata.get(TIMELINE_KEY).map(|s| s.as_str()) {
                Some(TIMELINE_TIME) => {
                    let timeline = Timeline::new(timeline.name.clone(), TimeType::Time);

                    let time = time
                        .as_any()
                        .downcast_ref::<Int64Array>()
                        .ok_or_else(|| anyhow!("expect time-like timeline to be a `Int64Array"))?;
                    ensure!(
                        time.len() == 1,
                        "expect only one timestamp per message per timeline"
                    );

                    Ok((timeline, time.values()[0].into()))
                }
                Some(TIMELINE_SEQUENCE) => {
                    let timeline = Timeline::new(timeline.name.clone(), TimeType::Sequence);

                    let time = time.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                        anyhow!("expect sequence-like timeline to be a `Int64Array")
                    })?;
                    ensure!(
                        time.len() == 1,
                        "expect only one timestamp per message per timeline"
                    );

                    Ok((timeline, time.values()[0].into()))
                }
                Some(unknown) => {
                    bail!("unknown timeline kind: {unknown:?}")
                }
                None => {
                    bail!("missing timeline kind")
                }
            },
        )
        .collect();

    timelines
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
        // TODO: explain why this cannot fail
        let (_, bucket) = self
            .buckets
            .range_mut(..=time)
            .rev()
            .take(1)
            .next()
            .unwrap();

        let size = bucket.total_size_bytes();
        let size_overflow = bucket.total_size_bytes() >= config.index_bucket_size_bytes;

        let len = bucket.total_rows();
        let len_overflow = len >= config.index_bucket_nb_rows;

        if size_overflow || len_overflow {
            debug!(
                timeline = %self.timeline.name(),
                entity = %self.ent_path,
                ?config,
                size,
                size_overflow,
                len,
                len_overflow,
                "allocating new index bucket, previous one overflowed"
            );

            if let Some(second_half) = bucket.split() {
                self.buckets.insert(second_half.time_range.min, second_half);
                return self.insert(config, time, indices);
            }
        }

        bucket.insert(time, indices)
    }
}

impl IndexBucket {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            time_range: TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            is_sorted: true,
            times: Int64Vec::default(),
            indices: Default::default(),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn insert(
        &mut self,
        time: TimeInt,
        row_indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        // append time to primary index
        self.times.push(time.as_i64().into());

        // append components to secondary indices (2-way merge)

        // 2-way merge, step1: left-to-right
        //
        // push new row indices to their associated secondary index
        for (name, row_idx) in row_indices {
            let index = self.indices.entry((*name).to_owned()).or_insert_with(|| {
                let mut index = UInt64Vec::default();
                index.extend_constant(self.times.len().saturating_sub(1), None);
                index
            });
            index.push(Some(*row_idx));
        }

        // 2-way merge, step2: right-to-left
        //
        // fill unimpacted secondary indices with null values
        for (name, index) in &mut self.indices {
            if !row_indices.contains_key(name.as_str()) {
                index.push_null();
            }
        }

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        // TODO(#433): re_datastore: properly handle already sorted data during insertion
        self.is_sorted = false;

        Ok(())
    }

    /// Splits the bucket in two, returning the second half.
    pub fn split(&mut self) -> Option<Self> {
        if self.times.len() < 2 {
            eprintln!("EARLY 1");
            return None; // early exit: can't split the unsplittable
        }

        self.sort_indices();

        // Used down the line to assert that we've left everything in a sane state.
        #[cfg(debug_assertions)]
        let times_len = self.times.len();
        #[cfg(debug_assertions)]
        let total_rows = self.total_rows();

        let Self {
            timeline,
            time_range: time_range1,
            is_sorted: _,
            times: times1,
            indices: indices1,
        } = self;

        let timeline = *timeline;
        // TODO: explain forward walk (and maybe express in a better way)
        let half_row = {
            let times = times1.values();

            let mut half_row = times1.len() / 2;
            let time = times[half_row];
            while half_row + 1 < times.len() && times[half_row] == time {
                half_row += 1;
            }

            half_row
        };

        // split existing time range in two, and create new time tange for the second half
        let time_range2 = TimeRange::new(times1.values()[half_row].into(), time_range1.max);
        // TODO: explain why this cannot underflow
        time_range1.max = times1.values()[half_row - 1].into();

        // The time index must always be dense, thus it shouldn't even have a validity
        // bitmap attached to it to begin with.
        debug_assert!(times1.validity().is_none());

        // split primary index in two, and build a new one for the second half
        let (datatype, mut data1, _) = std::mem::take(times1).into_data();
        let data2 = data1.split_off(half_row);
        *times1 = Int64Vec::from_data(datatype.clone(), data1, None);
        let times2 = Int64Vec::from_data(datatype.clone(), data2, None);

        #[cfg(debug_assertions)]
        {
            // both resulting time halves must be smaller or equal than the halfway point
            assert!(times1.len() <= half_row);
            assert!(times2.len() <= half_row);
            // both resulting halves must sum up to the length of the original time index
            assert_eq!(times_len, times1.len() + times2.len());
        }

        fn split_index_off(index1: &mut UInt64Vec, half_row: usize) -> UInt64Vec {
            let (datatype, mut data1, validity1) = std::mem::take(index1).into_data();
            let data2 = data1.split_off(half_row);
            if let Some((validity1, validity2)) = validity1.map(|validity1| {
                let mut validity1 = validity1.into_iter().collect::<Vec<_>>();
                let validity2 = validity1.split_off(half_row);
                (
                    MutableBitmap::from_iter(validity1),
                    MutableBitmap::from_iter(validity2),
                )
            }) {
                *index1 = UInt64Vec::from_data(datatype.clone(), data1, Some(validity1));
                UInt64Vec::from_data(datatype.clone(), data2, Some(validity2))
            } else {
                *index1 = UInt64Vec::from_data(datatype.clone(), data1, None);
                UInt64Vec::from_data(datatype.clone(), data2, None)
            }
        }

        // split all secondary indices in two, and build new ones for the second halves
        let indices2: HashMap<_, _> = indices1
            .iter_mut()
            .map(|(name, index1)| {
                let index2 = split_index_off(index1, half_row);

                #[cfg(debug_assertions)]
                {
                    // both resulting time halves must be smaller or equal than the halfway point
                    assert!(index1.len() <= half_row);
                    assert!(index2.len() <= half_row);
                    // both resulting halves must sum up to the length of the original time index
                    assert_eq!(times_len, index1.len() + index2.len());
                }

                ((*name).clone(), index2)
            })
            .collect();

        let second_half = Self {
            timeline,
            time_range: time_range2,
            is_sorted: true,
            times: times2,
            indices: indices2,
        };

        // sanity checks
        #[cfg(debug_assertions)]
        {
            self.sanity_check().unwrap();
            second_half.sanity_check().unwrap();

            let total_rows1 = self.total_rows() as i64;
            let total_rows2 = second_half.total_rows() as i64;

            assert!(total_rows1.abs_diff(total_rows2) < 2);
            assert_eq!(total_rows as i64, total_rows1 + total_rows2);
        }

        // return the second half!
        Some(second_half)
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
                name = self.name.as_str(),
                ?config,
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
        self.buckets.back_mut().unwrap().insert(timelines, data)
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
