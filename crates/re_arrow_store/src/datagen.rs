//! Generate random data for tests and benchmarks.

use std::{collections::BTreeMap, time::SystemTime};

use arrow2::{
    array::{Array, Float32Array, Int64Array, ListArray, PrimitiveArray, StructArray},
    buffer::Buffer,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema, TimeUnit},
};
use arrow2_convert::serialize::TryIntoArrow;
use re_log_types::arrow::{ENTITY_PATH_KEY, TIMELINE_KEY, TIMELINE_SEQUENCE, TIMELINE_TIME};
use re_log_types::ObjPath as EntityPath;

use crate::{field_types, ComponentNameRef, TypedTimeInt};

/// Wrap `field_array` in a single-element `ListArray`
pub fn wrap_in_listarray(field_array: Box<dyn Array>) -> ListArray<i32> {
    let datatype = ListArray::<i32>::default_datatype(field_array.data_type().clone());
    let offsets = Buffer::from(vec![0, field_array.len() as i32]);
    let values = field_array;
    let validity = None;
    ListArray::<i32>::from_data(datatype, offsets, values, validity)
}

/// Create `len` dummy rectangles
pub fn build_some_rects(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| field_types::Rect2D {
            x: i as f32,
            y: i as f32,
            w: (i / 2) as f32,
            h: (i / 2) as f32,
        })
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| i as field_types::ColorRGBA)
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Create `len` dummy labels
pub fn build_some_labels(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| format!("label{i}"))
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Build a sample row of Rect data
pub fn build_test_rect_chunk() -> (Chunk<Box<dyn Array>>, Schema) {
    let time = arrow2::array::UInt32Array::from_slice([1234]).boxed();
    let rect = wrap_in_listarray(build_some_rects(5)).boxed();
    let color = wrap_in_listarray(build_some_colors(5)).boxed();
    let label = wrap_in_listarray(build_some_labels(1)).boxed();

    let schema = vec![
        Field::new("log_time", time.data_type().clone(), false),
        Field::new("rect", rect.data_type().clone(), true),
        Field::new("color", color.data_type().clone(), true),
        Field::new("label", label.data_type().clone(), true),
    ]
    .into();
    let chunk = Chunk::new(vec![time, rect, color, label]);
    (chunk, schema)
}

pub fn build_log_time(log_time: SystemTime) -> (TypedTimeInt, Schema, Int64Array) {
    let log_time = log_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    let datatype = DataType::Timestamp(TimeUnit::Nanosecond, None);

    let data = PrimitiveArray::from([Some(log_time)]).to(datatype.clone());

    let fields = [Field::new("log_time", datatype, false)
        .with_metadata([(TIMELINE_KEY.to_owned(), TIMELINE_TIME.to_owned())].into())]
    .to_vec();

    let schema = Schema {
        fields,
        ..Default::default()
    };

    let time = TypedTimeInt::new_time(log_time);

    (time, schema, data)
}

pub fn build_frame_nr(frame_nr: i64) -> (TypedTimeInt, Schema, Int64Array) {
    let data = PrimitiveArray::from([Some(frame_nr)]);

    let fields = [Field::new("frame_nr", DataType::Int64, false)
        .with_metadata([(TIMELINE_KEY.to_owned(), TIMELINE_SEQUENCE.to_owned())].into())]
    .to_vec();

    let schema = Schema {
        fields,
        ..Default::default()
    };

    let time = TypedTimeInt::new_seq(frame_nr);

    (time, schema, data)
}

pub fn pack_timelines(
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

pub fn build_instances(nb_instances: usize) -> (ComponentNameRef<'static>, Schema, ListArray<i32>) {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let data = PrimitiveArray::from(
        (0..nb_instances)
            .into_iter()
            .map(|_| Some(rng.gen()))
            .collect::<Vec<Option<u32>>>(),
    );
    let data = wrap_in_listarray(data.boxed());

    let fields = [Field::new("instances", data.data_type().clone(), false)].to_vec();
    let schema = Schema {
        fields,
        ..Default::default()
    };

    ("instances", schema, data)
}

pub fn build_rects(nb_instances: usize) -> (ComponentNameRef<'static>, Schema, ListArray<i32>) {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let data = {
        let data: Box<[_]> = (0..nb_instances).into_iter().map(|_| rng.gen()).collect();
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
    let data = wrap_in_listarray(data.boxed());

    let fields = [Field::new("rects", data.data_type().clone(), false)].to_vec();
    let schema = Schema {
        fields,
        ..Default::default()
    };

    ("rects", schema, data)
}

pub fn build_positions(nb_instances: usize) -> (ComponentNameRef<'static>, Schema, ListArray<i32>) {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let data = {
        let xs: Box<[_]> = (0..nb_instances)
            .into_iter()
            .map(|_| rng.gen_range(0.0..10.0))
            .collect();
        let ys: Box<[_]> = (0..nb_instances)
            .into_iter()
            .map(|_| rng.gen_range(0.0..10.0))
            .collect();
        let x = Float32Array::from_slice(&xs).boxed();
        let y = Float32Array::from_slice(&ys).boxed();
        let fields = vec![
            Field::new("x", DataType::Float32, false),
            Field::new("y", DataType::Float32, false),
        ];
        StructArray::new(DataType::Struct(fields), vec![x, y], None)
    };
    let data = wrap_in_listarray(data.boxed());

    let fields = [Field::new("positions", data.data_type().clone(), false)].to_vec();
    let schema = Schema {
        fields,
        ..Default::default()
    };

    ("positions", schema, data)
}

pub fn pack_components(
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

pub fn build_message(
    ent_path: &EntityPath,
    timelines: impl IntoIterator<Item = (TypedTimeInt, Schema, Int64Array)>,
    components: impl IntoIterator<Item = (ComponentNameRef<'static>, Schema, ListArray<i32>)>,
) -> (Schema, Chunk<Box<dyn Array>>) {
    let mut schema = Schema::default();
    let mut cols: Vec<Box<dyn Array>> = Vec::new();

    schema.metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), ent_path.to_string())]);

    // Build & pack timelines
    let (timelines_schema, timelines_data) = pack_timelines(
        timelines
            .into_iter()
            .map(|(_, schema, data)| (schema, data.boxed())),
    );
    schema.fields.extend(timelines_schema.fields);
    schema.metadata.extend(timelines_schema.metadata);
    cols.push(timelines_data.boxed());

    // Build & pack components
    let (components_schema, components_data) = pack_components(
        components
            .into_iter()
            .map(|(_, schema, data)| (schema, data.boxed())),
    );
    schema.fields.extend(components_schema.fields);
    schema.metadata.extend(components_schema.metadata);
    cols.push(components_data.boxed());

    (schema, Chunk::new(cols))
}
