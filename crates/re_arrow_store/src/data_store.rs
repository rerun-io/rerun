use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, bail, ensure};
use arrow2::array::{new_empty_array, Array, Int64Array, ListArray, PrimitiveArray, StructArray};
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::compute::concatenate::concatenate;
use arrow2::datatypes::{DataType, Schema};
use nohash_hasher::IntMap;

use polars::prelude::IndexOfSchema;
use re_log_types::arrow::{
    filter_time_cols, ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME,
};
use re_log_types::{ObjPath as EntityPath, TimeInt, TimeType, Timeline};

// TODO: going for the usual principles here:
// - be liberal in what you accept, be strict in what you return
// - 1) make it work 2) make it correct (i.e. _tested_) 3) make it fast

// TODO:
// - write path
// - read path
// - purge / GC (later)

// TODO:
// - keeping low level _for now_ (i.e. no polars at this layer)
//    - need to get familiar with what's actually going on under the good
//    - don't add layers until we have a use case for them

// ---

// https://www.notion.so/rerunio/Arrow-Table-Design-cd77528c77ae4aa4a8c566e2ec29f84f

// TODO: perf probes
// TODO: every error and assert paths must be _TESTED_!!!

type ComponentName = String;
type ComponentNameRef<'a> = &'a str;
type RowIndex = u64;
type TimeIntRange = std::ops::Range<TimeInt>;

/// The complete data store: covers all timelines, all entities, everything.
#[derive(Default)]
pub struct DataStore {
    /// Maps an entity to its index, for a specific timeline.
    indices: HashMap<(Timeline, EntityPath), IndexTable>,
    /// Maps a component to its data, for all timelines and all entities.
    components: HashMap<ComponentName, ComponentTable>,
}

impl DataStore {
    //     fn insert_components(&mut self, timeline, time, obj_path,
    //         components: Map<ComponentName, ArrowStore>) {
    //         let instance_row = self.components["instance_keys"].push(instance_keys);
    //         let pos_row = self.components["positions"].push(positions);
    //         self.main_tables[(timeline, obj_path)]
    //             .insert(time, instance_row, pos_row);
    //     }
    pub fn insert(&mut self, schema: &Schema, msg: Chunk<Box<dyn Array>>) -> anyhow::Result<()> {
        dbg!(&schema);
        dbg!(&msg);

        let ent_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| EntityPath::from(path.as_str()))?;

        let timelines = extract_timelines(schema, &msg)?;

        // TODO: Let's start the very dumb way: one bucket per TimeInt, then we'll deal with
        // actual ranges.
        for (timeline, time) in &timelines {
            dbg!((&ent_path, timeline));
            let index = self
                .indices
                .entry((timeline.clone(), ent_path.clone()))
                .or_insert(Default::default());
        }

        let components = extract_components(schema, &msg)?;
        dbg!(&components);

        // let mut indices = HashMap::with_capacity(components.len());
        for (name, component) in components {
            let table = self
                .components
                .entry(name.to_owned())
                .or_insert(ComponentTable::new(component.data_type().clone()));

            let row_idx = table.insert(&timelines, component);
            dbg!(row_idx);
        }

        Ok(())
    }

    pub fn query() {}
}

// TODO: document the datamodel here: 1 timestamp per message per timeline.
fn extract_timelines<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(Timeline, TimeInt)>> {
    let timelines = schema
        .index_of("timelines") // TODO
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `timelines` field`"))?;

    let timelines = timelines
        .as_any()
        .downcast_ref::<ListArray<i32>>()
        .ok_or_else(|| anyhow!("expect top-level `timelines` to be a `ListArray<i32>`"))?;

    let timelines = timelines
        .values()
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect timeline values to be `StructArray`s"))?;

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

                    Ok((timeline, TimeInt::from(time.values()[0])))
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

                    Ok((timeline, TimeInt::from(time.values()[0])))
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
) -> anyhow::Result<Vec<(ComponentNameRef<'data>, &'data Box<dyn Array>)>> {
    let components = schema
        .index_of("components") // TODO
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<ListArray<i32>>()
        .ok_or_else(|| anyhow!("expect top-level `components` to be a `ListArray<i32>`"))?;

    let components = components
        .values()
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    // TODO: check validity using component registry and such
    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| (field.name.as_str(), comp))
        .collect())
}

/// A chunked index, bucketized over time and space (whichever comes first).
///
/// Each bucket covers a half-open time range.
/// These time ranges are guaranteed to be non-overlapping.
///
/// ```text
/// Bucket #1: #202..#206
///
/// time | instances | comp#1 | comp#2 | … | comp#N |
/// ---------------------------------------|--------|
/// #202 | 2         | 2      | -      | … | 1      |
/// #203 | 3         | -      | 3      | … | 4      |
/// #204 | 4         | 6      | -      | … | -      |
/// #204 | 4         | 8      | 8      | … | -      |
/// #205 | 0         | 0      | 0      | … | -      |
/// #205 | 5         | -      | 9      | … | 2      |
/// ```
///
/// TODO:
/// - talk about out of order data and the effect it has
/// - talk about deletion
/// - talk about _lack of_ smallvec optimization
/// - talk (and test) append-only behavior
///
/// See also: [`Self::IndexBucket`].
//
//
// Each entry is a row index. It's nullable, with `null` = no entry.
#[derive(Default)]
pub struct IndexTable {
    buckets: BTreeMap<TimeInt, IndexBucket>,
}

impl IndexTable {
    // impl Index {
    //     pub fn insert(&mut self, time, instance_row, pos_row) {
    //         self.find_batch(time).insert(time, instance_row, pos_row)
    //     }

    //     pub fn find_batch(&mut self, time) {
    //         if let Some(bucket) = self.range(time..).next() {
    //             // if it is too big, split it in two
    //         } else {
    //             // create new bucket
    //         }
    //     }
    // }
}

/// TODO
//
// Has a max size of 128MB OR 10k rows, whatever comes first.
// The size-limit is so we can purge memory in small buckets
// The row-limit is to avoid slow re-sorting at query-time
pub struct IndexBucket {
    /// The time range covered by this bucket.
    time_range: TimeIntRange,

    /// All indices for this bucket.
    ///
    /// Each column in this dataframe corresponds to a component.
    //
    // new columns may be added at any time
    // sorted by the first column, time (if [`Self::is_sorted`])
    //
    // TODO(cmc): some components are always present: timelines, instances
    indices: (),
}

/// A chunked component table (i.e. a single column), bucketized by size only.
//
// The ComponentTable maps a row index to a list of values (e.g. a list of colors).
pub struct ComponentTable {
    /// Each bucket covers an arbitrary range of rows.
    /// How large is that range will depend on the size of the actual data, which is the actual
    /// trigger for chunking.
    buckets: BTreeMap<RowIndex, ComponentBucket>,
}

impl ComponentTable {
    fn new(datatype: DataType) -> Self {
        ComponentTable {
            buckets: [(0, ComponentBucket::new(datatype, 0))].into(),
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
        timelines: &[(Timeline, TimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        // TODO: Let's start the very dumb way: one bucket only, then we'll deal with splitting.
        self.buckets.get_mut(&0).unwrap().insert(timelines, data)
    }
}

/// TODO
//
// Has a max-size of 128MB or so.
// We bucket the component table so we can purge older parts when needed.
pub struct ComponentBucket {
    /// The time ranges (plural!) covered by this bucket.
    ///
    /// Buckets are never sorted over time, time ranges can grow arbitrarily large.
    //
    // Used when to figure out if we can purge it.
    // Out-of-order inserts can create huge time ranges here,
    // making some buckets impossible to purge, but we accept that risk.
    //
    // TODO: this is for purging only
    time_ranges: HashMap<Timeline, TimeIntRange>,

    // TODO
    row_offset: RowIndex,

    /// All the data for this bucket. This is a single column!
    ///
    /// Each row contains the data for all instances.
    /// Instances within a row are sorted
    //
    // maps a row index to a list of values (e.g. a list of colors).
    //
    // TODO: growable array
    data: Box<dyn Array>,
}

impl ComponentBucket {
    pub fn new(datatype: DataType, row_offset: RowIndex) -> Self {
        Self {
            row_offset,
            time_ranges: Default::default(),
            data: new_empty_array(dbg!(datatype)),
        }
    }

    pub fn insert(
        &mut self,
        timelines: &[(Timeline, TimeInt)],
        data: &Box<dyn Array>,
    ) -> anyhow::Result<RowIndex> {
        for (timeline, time) in timelines {
            // prob should own it at this point
            let time = *time;
            let time_plus_one = time + TimeInt::from(1);
            self.time_ranges
                .entry(timeline.clone())
                .and_modify(|range| *range = range.start.min(time)..range.end.max(time_plus_one))
                .or_insert_with(|| time..time_plus_one);
        }

        // TODO: actual mutable array :)
        self.data = concatenate(&[&*self.data, &**data])?;
        dbg!(&self.data);

        Ok(self.row_offset + self.data.len() as u64 - 1)
    }
}

// TODO: scenarios
// - insert a single component for a single instance and query it back
// - insert a single component at t1 then another one at t2 then query at t0, t1, t2, t3
//
// TODO: messy ones
// - multiple components, different number of rows or something
#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Instant, SystemTime},
    };

    use arrow2::{
        array::{Array, Float32Array, ListArray, PrimitiveArray, StructArray, UInt32Array},
        buffer::Buffer,
        chunk::Chunk,
        datatypes::{self, DataType, Field, Schema, TimeUnit},
    };
    use polars::export::num::ToPrimitive;
    use re_log_types::arrow::{TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME};

    use super::*;

    #[test]
    fn single_entity_single_component_roundtrip() {
        fn build_log_time(log_time: SystemTime) -> (Schema, Int64Array) {
            let log_time = log_time
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_u64()
                .unwrap();

            let datatype = DataType::Timestamp(TimeUnit::Nanosecond, None);

            let data = PrimitiveArray::from([Some(log_time as i64)]).to(datatype.clone());

            let fields = [Field::new("log_time", datatype, false)
                .with_metadata([(TIMELINE_KEY.to_owned(), TIMELINE_TIME.to_owned())].into())]
            .to_vec();

            let schema = Schema {
                fields,
                ..Default::default()
            };

            (schema, data)
        }

        fn build_frame_nr(frame_nr: i64) -> (Schema, Int64Array) {
            let data = PrimitiveArray::from([Some(frame_nr)]);

            let fields = [Field::new("frame_nr", DataType::Int64, false)
                .with_metadata([(TIMELINE_KEY.to_owned(), TIMELINE_SEQUENCE.to_owned())].into())]
            .to_vec();

            let schema = Schema {
                fields,
                ..Default::default()
            };

            (schema, data)
        }

        // TODO: implicit assumption here is that one message = one timestamp per timeline, i.e.
        // you can send data for multiple times in one single message.
        fn pack_timelines(
            timelines: impl Iterator<Item = (Schema, Box<dyn Array>)>,
        ) -> (Schema, ListArray<i32>) {
            let (timeline_schemas, timeline_cols): (Vec<_>, Vec<_>) = timelines.unzip();
            let timeline_fields = timeline_schemas
                .into_iter()
                .flat_map(|schema| schema.fields)
                .collect();
            let packed = StructArray::new(DataType::Struct(timeline_fields), timeline_cols, None);

            let packed = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(packed.data_type().clone()), // datatype
                Buffer::from(vec![0, 1i32]),                                    // offsets
                packed.boxed(),                                                 // values
                None,                                                           // validity
            );

            let schema = Schema {
                fields: [Field::new("timelines", packed.data_type().clone(), false)].to_vec(),
                ..Default::default()
            };

            (schema, packed)
        }

        fn build_instances(nb_rows: usize) -> (Schema, UInt32Array) {
            use rand::Rng as _;

            let mut rng = rand::thread_rng();
            let data = PrimitiveArray::from(
                (0..nb_rows)
                    .into_iter()
                    .map(|_| Some(rng.gen()))
                    .collect::<Vec<Option<u32>>>(),
            );

            let fields = [Field::new("instances", data.data_type().clone(), false)].to_vec();

            let schema = Schema {
                fields,
                ..Default::default()
            };

            (schema, data)
        }

        fn build_rects(nb_rows: usize) -> (Schema, StructArray) {
            let data = {
                let data: Box<[_]> = (0..nb_rows).into_iter().map(|i| i as f32 / 10.0).collect();
                let x = Float32Array::from_slice(&data).boxed();
                let y = Float32Array::from_slice(&data).boxed();
                let w = Float32Array::from_slice(&data).boxed();
                let h = Float32Array::from_slice(&data).boxed();
                let fields = vec![
                    Field::new("x", DataType::Float32, false),
                    Field::new("y", DataType::Float32, false),
                    Field::new("w", DataType::Float32, false),
                    Field::new("h", DataType::Float32, false),
                ];
                StructArray::new(DataType::Struct(fields), vec![x, y, w, h], None)
            };

            let fields = [Field::new("rect", data.data_type().clone(), false)].to_vec();

            let schema = Schema {
                fields,
                ..Default::default()
            };

            (schema, data)
        }

        fn pack_components(
            components: impl Iterator<Item = (Schema, Box<dyn Array>)>,
        ) -> (Schema, ListArray<i32>) {
            let (component_schemas, component_cols): (Vec<_>, Vec<_>) = components.unzip();
            let component_fields = component_schemas
                .into_iter()
                .flat_map(|schema| schema.fields)
                .collect();

            let nb_rows = component_cols[0].len();
            let packed = StructArray::new(DataType::Struct(component_fields), component_cols, None);

            let packed = ListArray::<i32>::from_data(
                ListArray::<i32>::default_datatype(packed.data_type().clone()), // datatype
                Buffer::from(vec![0, nb_rows as i32]),                          // offsets
                packed.boxed(),                                                 // values
                None,                                                           // validity
            );

            let schema = Schema {
                fields: [Field::new("components", packed.data_type().clone(), false)].to_vec(),
                ..Default::default()
            };

            (schema, packed)
        }

        fn build_message(ent_path: &EntityPath, nb_rows: usize) -> (Schema, Chunk<Box<dyn Array>>) {
            let mut schema = Schema::default();
            let mut cols: Vec<Box<dyn Array>> = Vec::new();

            schema.metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), ent_path.to_string())]);

            // Build & pack timelines
            let (log_time_schema, log_time_data) = build_log_time(SystemTime::now());
            let (frame_nr_schema, frame_nr_data) = build_frame_nr(42);
            let (timelines_schema, timelines_data) = pack_timelines(
                [
                    (log_time_schema, log_time_data.boxed()),
                    (frame_nr_schema, frame_nr_data.boxed()),
                ]
                .into_iter(),
            );
            schema.fields.extend(timelines_schema.fields);
            schema.metadata.extend(timelines_schema.metadata);
            cols.push(timelines_data.boxed());

            // Build & pack components
            // TODO: what about when nb_rows differs between components? is that legal?
            let (instances_schema, instances_data) = build_instances(nb_rows);
            let (rects_schema, rects_data) = build_rects(nb_rows);
            let (components_schema, components_data) = pack_components(
                [
                    (instances_schema, instances_data.boxed()),
                    (rects_schema, rects_data.boxed()),
                ]
                .into_iter(),
            );
            schema.fields.extend(components_schema.fields);
            schema.metadata.extend(components_schema.metadata);
            cols.push(components_data.boxed());

            (schema, Chunk::new(cols))
        }

        let mut store = DataStore::default();

        let ent_path = EntityPath::from("this/that");
        let (schema, components) = build_message(&ent_path, 10);
        // dbg!(schema);
        // dbg!(components);

        store.insert(&schema, components).unwrap();
    }
}
