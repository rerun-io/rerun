// TODO: scenarios
// - insert a single component for a single instance and query it back
// - insert a single component at t1 then another one at t2 then query at t0, t1, t2, t3
// - send one message with multiple lists vs. multiple messages with 1/N lists
//
// TODO: messy ones
// - multiple components, different number of rows or something

use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime},
};

use arrow2::{
    array::{Array, Float32Array, Int64Array, ListArray, PrimitiveArray, StructArray, UInt32Array},
    buffer::Buffer,
    chunk::Chunk,
    datatypes::{self, DataType, Field, Schema, TimeUnit},
};
use polars::export::num::ToPrimitive;

use re_arrow_store::DataStore;
use re_log_types::arrow::{
    filter_time_cols, ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME,
};
use re_log_types::{ObjPath as EntityPath, TimeInt, TimeType, Timeline};

// ---

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
    ) -> (Schema, StructArray) {
        let (timeline_schemas, timeline_cols): (Vec<_>, Vec<_>) = timelines.unzip();
        let timeline_fields = timeline_schemas
            .into_iter()
            .flat_map(|schema| schema.fields)
            .collect();
        let packed = StructArray::new(DataType::Struct(timeline_fields), timeline_cols, None);

        let schema = Schema {
            fields: [Field::new("timelines", packed.data_type().clone(), false)].to_vec(),
            ..Default::default()
        };

        (schema, packed)
    }

    fn build_instances(nb_rows: usize) -> (Schema, ListArray<i32>) {
        use rand::Rng as _;

        let mut rng = rand::thread_rng();
        let data = PrimitiveArray::from(
            (0..nb_rows)
                .into_iter()
                .map(|_| Some(rng.gen()))
                .collect::<Vec<Option<u32>>>(),
        );

        let data = ListArray::<i32>::from_data(
            ListArray::<i32>::default_datatype(data.data_type().clone()), // datatype
            Buffer::from(vec![0, nb_rows as i32]),                        // offsets
            data.boxed(),                                                 // values
            None,                                                         // validity
        );

        let fields = [Field::new("instances", data.data_type().clone(), false)].to_vec();

        let schema = Schema {
            fields,
            ..Default::default()
        };

        (schema, data)
    }

    fn build_rects(nb_rows: usize) -> (Schema, ListArray<i32>) {
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

        let data = ListArray::<i32>::from_data(
            ListArray::<i32>::default_datatype(data.data_type().clone()), // datatype
            Buffer::from(vec![0, nb_rows as i32]),                        // offsets
            data.boxed(),                                                 // values
            None,                                                         // validity
        );

        let fields = [Field::new("rect", data.data_type().clone(), false)].to_vec();

        let schema = Schema {
            fields,
            ..Default::default()
        };

        (schema, data)
    }

    fn pack_components(
        components: impl Iterator<Item = (Schema, Box<dyn Array>)>,
    ) -> (Schema, StructArray) {
        let (component_schemas, component_cols): (Vec<_>, Vec<_>) = components.unzip();
        let component_fields = component_schemas
            .into_iter()
            .flat_map(|schema| schema.fields)
            .collect();

        let packed = StructArray::new(DataType::Struct(component_fields), component_cols, None);

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
    store.insert(&schema, components).unwrap();

    std::thread::sleep(Duration::from_millis(10));

    let (schema, components) = build_message(&ent_path, 20);
    store.insert(&schema, components).unwrap();
}
