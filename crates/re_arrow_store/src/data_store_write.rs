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
use re_log_types::{ObjPath as EntityPath, TimeType, Timeline};

use crate::{
    ComponentBucket, ComponentNameRef, ComponentTable, DataStore, IndexBucket, IndexTable,
    RowIndex, TypedTimeInt,
};

// --- Data store ---

impl DataStore {
    /// Inserts a payload of Arrow data into the datastore.
    ///
    /// The payload is expected to hold:
    /// - the entity path,
    /// - the targeted timelines & timepoints,
    /// - and all the components data.
    pub fn insert(&mut self, schema: &Schema, msg: Chunk<Box<dyn Array>>) -> anyhow::Result<()> {
        let ent_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| EntityPath::from(path.as_str()))?;

        let timelines = extract_timelines(schema, &msg)?;
        let components = extract_components(schema, &msg)?;

        // TODO: sort the "instances" component, and everything else accordingly!

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
                .entry((timeline.clone(), ent_path.clone()))
                .or_insert_with(|| IndexTable::new(timeline.clone(), ent_path.clone()));
            index.insert(*time, &indices)?;
        }

        Ok(())
    }
}

// TODO: document the datamodel here: 1 timestamp per message per timeline.
// TODO: is that the right data model for this? is it optimal? etc
fn extract_timelines<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(Timeline, TypedTimeInt)>> {
    let timelines = schema
        .index_of("timelines") // TODO
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

                    Ok((
                        timeline,
                        TypedTimeInt::from((TimeType::Time, time.values()[0])),
                    ))
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

                    Ok((
                        timeline,
                        TypedTimeInt::from((TimeType::Sequence, time.values()[0])),
                    ))
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

// TODO: is that the right data model for this? is it optimal? etc
fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(ComponentNameRef<'data>, &'data Box<dyn Array>)>> {
    let components = schema
        .index_of("components") // TODO
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    // TODO: check validity using component registry and such
    // TODO: they all should be list, no matter what!!!
    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| (field.name.as_str(), comp))
        .collect())
}

// --- Indices ---

impl IndexTable {
    pub fn new(timeline: Timeline, ent_path: EntityPath) -> Self {
        Self {
            timeline,
            ent_path,
            buckets: [(
                TypedTimeInt::from((timeline.typ(), 0)),
                IndexBucket::new(timeline),
            )]
            .into(),
        }
    }

    pub fn insert(
        &mut self,
        time: TypedTimeInt,
        indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        // TODO: real bucketing!
        let bucket = self.buckets.iter_mut().next().unwrap().1;
        bucket.insert(time, indices)
    }
}

impl IndexBucket {
    pub fn new(timeline: Timeline) -> Self {
        let start = TypedTimeInt::from((timeline.typ(), i64::MIN));
        let end = TypedTimeInt::from((timeline.typ(), i64::MAX));
        Self {
            time_range: start..end,
            is_sorted: true,
            times: Int64Vec::default(),
            indices: Default::default(),
        }
    }

    pub fn insert(
        &mut self,
        time: TypedTimeInt,
        row_indices: &HashMap<ComponentNameRef<'_>, RowIndex>,
    ) -> anyhow::Result<()> {
        // append time
        self.times.push(time.as_i64().into());

        // append everything else!

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
            let index = self.indices.entry(name.to_string()).or_insert_with(|| {
                let mut index = UInt64Vec::default();
                index.extend_constant(self.times.len().saturating_sub(1), None);
                index
            });
            index.push(Some(*row_idx))
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
            assert!(self
                .indices
                .values()
                .map(|index| index.len())
                .all(|len| len == expected_len));
        }

        self.is_sorted = false;
        self.sort_indices()?; // TODO: move to read path!

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

    //     pub fn push(&mut self, time_points, values) -> u64 {
    //         if self.last().len() > TOO_LARGE {
    //             self.push(ComponentTableBucket::new());
    //         }
    //         self.last().push(time_points, values)
    //     }
    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TypedTimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        // TODO: Let's start the very dumb way: one bucket only, then we'll deal with splitting.
        // TODO: real bucketing!
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
                _ => todo!("throw an error here, this should always be a list"), // TODO
            };

            let empty = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(inner_datatype.clone()),
                Buffer::from(vec![0, 0 as i32]),
                new_empty_array(inner_datatype),
                None,
            );

            // TODO: throw error
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
        timelines: &[(Timeline, TypedTimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        for (timeline, time) in timelines {
            // TODO: prob should own it at this point
            let time = *time;
            let time_plus_one = time + 1;
            self.time_ranges
                .entry(timeline.clone())
                .and_modify(|range| *range = range.start.min(time)..range.end.max(time_plus_one))
                .or_insert_with(|| time..time_plus_one);
        }

        // TODO: actual mutable array :)
        self.data = concatenate(&[&*self.data, &**data])?;

        Ok(self.row_offset + self.data.len() as u64 - 1)
    }
}
