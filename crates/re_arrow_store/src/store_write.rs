use std::collections::HashMap;
use std::sync::Arc;

use arrow2::array::{new_empty_array, Array, Int64Vec, ListArray, MutableArray, UInt64Vec};
use arrow2::buffer::Buffer;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::DataType;

use re_log::debug;
use re_log_types::{
    ComponentNameRef, ObjPath as EntityPath, TimeInt, TimePoint, TimeRange, Timeline,
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
    pub fn insert(
        &mut self,
        ent_path: &EntityPath,
        time_point: &TimePoint,
        components: impl ExactSizeIterator<Item = (ComponentNameRef<'static>, Box<dyn Array>)>,
    ) -> anyhow::Result<()> {
        // TODO(cmc): sort the "instances" component, and everything else accordingly!
        let ent_path_hash = *ent_path.hash();

        let mut indices = HashMap::with_capacity(components.len());
        for (name, component) in components {
            let table = self.components.entry(name.to_owned()).or_insert_with(|| {
                ComponentTable::new(name.to_owned(), component.data_type().clone())
            });

            let row_idx = table.insert(&self.config, time_point.iter(), component.as_ref())?;
            indices.insert(name, row_idx);
        }

        for (timeline, time) in time_point.iter() {
            let ent_path = ent_path.clone(); // shallow
            let index = self
                .indices
                .entry((*timeline, ent_path_hash))
                .or_insert_with(|| IndexTable::new(*timeline, ent_path));
            index.insert(*time, &indices)?;
        }

        Ok(())
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
        time: TimeInt,
        indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        let bucket = self.buckets.iter_mut().next().unwrap().1;
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

        // All indices (+ time!) should always have the exact same length.
        {
            let expected_len = self.times.len();
            debug_assert!(self
                .indices
                .values()
                .map(|index| index.len())
                .all(|len| len == expected_len));
        }

        // TODO(#433): re_datastore: properly handle already sorted data during insertion
        self.is_sorted = false;

        Ok(())
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

    pub fn insert<'a>(
        &mut self,
        config: &DataStoreConfig,
        timelines: impl IntoIterator<Item = (&'a Timeline, &'a TimeInt)>,
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
    pub fn insert<'a>(
        &mut self,
        timelines: impl IntoIterator<Item = (&'a Timeline, &'a TimeInt)>,
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
