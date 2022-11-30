use std::collections::{BTreeMap, HashMap};

use arrow2::array::{Array, ListArray, StructArray};
use arrow2::buffer::Buffer;
use arrow2::chunk::Chunk;
use arrow2::datatypes::Schema;
use nohash_hasher::IntMap;
use re_log_types::arrow::{filter_time_cols, ENTITY_PATH_KEY};
use re_log_types::{ArrowMsg, FieldName, ObjPath as EntityPath, TimeInt, Timeline};

// TODO: feels like arrow schema shouldn't have top-level fields at all.
//
// today it looks like this:
// - "timeline #1"
// - "timeline #2"
// - "timeline #N"
// - "components"
//    - "component #1"
//    - "component #2"
//    - "component #N"
//
// i think it might as well look like this:
// - "timelines"
//    - "timeline #1"
//    - "timeline #2"
//    - "timeline #N"
// - "components"
//    - "component #1"
//    - "component #2"
//    - "component #N"

// TODO:
// - write path
// - read path
// - purge / GC

// ---

// https://www.notion.so/rerunio/Arrow-Table-Design-cd77528c77ae4aa4a8c566e2ec29f84f

type ComponentName = String;
type RowIndex = u64;
type TimeIntRange = std::ops::Range<TimeInt>;

/// The complete data store: covers all timelines, all entities, everything.
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
    pub fn insert(
        &mut self,
        ent_path: &EntityPath,
        schema: &Schema,
        chunk: Chunk<Box<dyn Array>>,
        components: &[Box<dyn Array>],
    ) -> anyhow::Result<()> {
        // 1. fetch the index
        //
        // 2. for each component:
        // 2.1. fetch the

        //

        Ok(())
    }
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
    //     pub fn push(&mut self, time_points, values) -> u64 {
    //         if self.last().len() > TOO_LARGE {
    //             self.push(ComponentTableBucket::new());
    //         }
    //         self.last().push(time_points, values)
    //     }
    pub fn insert() {}
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
    // TODO(cmc): do we really need a dataframe or should this just be some raw arrow data?
    data: (),
}

// TODO: scenarios
// - insert a single component for a single instance and query it back
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
        array::{Array, Float32Array, ListArray, PrimitiveArray, StructArray},
        buffer::Buffer,
        chunk::Chunk,
        datatypes::{self, DataType, Field, Schema, TimeUnit},
    };
    use re_log_types::arrow::{TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME};

    use super::*;

    #[test]
    fn single_entity_single_component_roundtrip() {
        // TODO: go for the usual "be liberal in what you accept, be strict in what you give back"

        fn build_log_time(log_time: SystemTime) -> (Schema, PrimitiveArray<i64>) {
            let log_time = log_time
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let datatype = DataType::Timestamp(TimeUnit::Second, None);

            let data = PrimitiveArray::from([Some(log_time as i64)]).to(datatype.clone());

            let fields = [
                Field::new("log_time", datatype, false).with_metadata(BTreeMap::from([(
                    TIMELINE_KEY.to_owned(),
                    TIMELINE_TIME.to_owned(),
                )])),
            ]
            .to_vec();

            let schema = Schema {
                fields,
                ..Default::default()
            };

            (schema, data)
        }

        fn build_frame_nr(frame_nr: i64) -> (Schema, PrimitiveArray<i64>) {
            let data = PrimitiveArray::from([Some(frame_nr)]);

            let fields = [
                Field::new("frame_nr", DataType::Int64, false).with_metadata(BTreeMap::from([(
                    TIMELINE_KEY.to_owned(),
                    TIMELINE_SEQUENCE.to_owned(),
                )])),
            ]
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

        fn build_instances(nb_rows: usize) -> (Schema, PrimitiveArray<u32>) {
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

        // TODO: probably shouldn't be a struct, it's whatever for now.
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

        let ent_path = EntityPath::from("this/that");
        let (schema, chunk) = build_message(&ent_path, 10);
        dbg!(schema);
        dbg!(chunk);
    }
}
