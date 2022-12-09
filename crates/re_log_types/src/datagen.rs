//! Generate random data for tests and benchmarks.

use crate::msg_bundle::{wrap_in_listarray, ComponentBundle};
use crate::{Time, TimeInt, TimeType, Timeline};
use arrow2::{
    array::{Array, Float32Array, PrimitiveArray, StructArray},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};
use arrow2_convert::serialize::TryIntoArrow;

use crate::field_types;

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
        .map(|i| field_types::ColorRGBA(i as u32))
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

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`TimePoint`].
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
    (Timeline::new("log_time", TimeType::Time), log_time.into())
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`TimePoint`].
pub fn build_frame_nr(frame_nr: i64) -> (Timeline, TimeInt) {
    (
        Timeline::new("frame_nr", TimeType::Sequence),
        frame_nr.into(),
    )
}

pub fn build_instances(nb_instances: usize) -> ComponentBundle<'static> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let data = PrimitiveArray::from(
        (0..nb_instances)
            .into_iter()
            .map(|_| Some(rng.gen()))
            .collect::<Vec<Option<u32>>>(),
    );
    let data = wrap_in_listarray(data.boxed());

    let field = Field::new("instances", data.data_type().clone(), false);

    ComponentBundle {
        name: "instances",
        field,
        component: data.boxed(),
    }
}

pub fn build_rects(nb_instances: usize) -> ComponentBundle<'static> {
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

    let field = Field::new("rects", data.data_type().clone(), false);

    ComponentBundle {
        name: "rects",
        field,
        component: data.boxed(),
    }
}

pub fn build_positions(nb_instances: usize) -> ComponentBundle<'static> {
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

    let field = Field::new("positions", data.data_type().clone(), false);

    ComponentBundle {
        name: "positions",
        field,
        component: data.boxed(),
    }
}
