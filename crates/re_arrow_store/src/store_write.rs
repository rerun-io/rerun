use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure};
use arrow2::array::{
    new_empty_array, Array, Int64Array, Int64Vec, ListArray, MutableArray, StructArray, UInt64Vec,
};
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::{DataType, Schema};
use polars::prelude::IndexOfSchema;

use re_log_types::arrow::{ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME};
use re_log_types::{ObjPath as EntityPath, TimeInt, TimeRange, TimeType, Timeline};

use crate::{
    ComponentBucket, ComponentNameRef, ComponentTable, DataStore, IndexBucket, IndexTable, RowIndex,
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

        let timelines = extract_timelines(schema, msg)?;
        let components = extract_components(schema, msg)?;

        // TODO(cmc): sort the "instances" component, and everything else accordingly!

        let mut indices = HashMap::with_capacity(components.len());
        for (name, component) in components {
            let table = self.components.entry(name.to_owned()).or_insert_with(|| {
                ComponentTable::new(name.to_owned(), component.data_type().clone())
            });

            let row_idx = table.insert(&timelines, component)?;
            indices.insert(name, row_idx);
        }

        for (timeline, time) in &timelines {
            let index = self
                .indices
                .entry((*timeline, ent_path.clone()))
                .or_insert_with(|| IndexTable::new(*timeline, ent_path.clone()));
            index.insert(*time, &indices)?;
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

        // Step 1: for all row indices, check whether the index for the associated component
        // exists:
        // - if it does, append the new row index to it
        // - otherwise, create a new one, fill it with nulls, and append the new row index to it
        //
        // After this step, we are guaranteed that all new row indices have been inserted into
        // the components' indices.
        //
        // What we are _not_ guaranteed, though, is that existing component indices that weren't
        // affected by this update are appended with null values so that they stay aligned with
        // the length of the time index.
        // Step 2 below takes care of that.
        for (name, row_idx) in row_indices {
            let index = self.indices.entry((*name).to_owned()).or_insert_with(|| {
                let mut index = UInt64Vec::default();
                index.extend_constant(self.times.len().saturating_sub(1), None);
                index
            });
            index.push(Some(*row_idx));
        }

        // Step 2: for all component indices, check whether they were affected by the current
        // insertion:
        // - if they weren't, append null values appropriately
        // - otherwise, do nothing, step 1 already took care of it
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
            buckets: [(0, ComponentBucket::new(name, datatype, 0))].into(),
        }
    }

    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TimeInt)],
        data: &dyn Array,
    ) -> anyhow::Result<RowIndex> {
        self.buckets.get_mut(&0).unwrap().insert(timelines, data)
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
