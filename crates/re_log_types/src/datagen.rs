//! Generate random data for tests and benchmarks.

use crate::msg_bundle::{wrap_in_listarray, ComponentBundle};
use crate::{Time, TimeInt, TimeType, Timeline};
use arrow2::{
    array::{Float32Array, PrimitiveArray, StructArray},
    datatypes::{DataType, Field},
};

use crate::field_types::{self};

/// Create `len` dummy rectangles
pub fn build_some_rects(len: usize) -> Vec<field_types::Rect2D> {
    (0..len)
        .into_iter()
        .map(|i| field_types::Rect2D {
            x: i as f32,
            y: i as f32,
            w: (i / 2) as f32,
            h: (i / 2) as f32,
        })
        .collect()
}

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Vec<field_types::ColorRGBA> {
    (0..len)
        .into_iter()
        .map(|i| field_types::ColorRGBA(i as u32))
        .collect()
}

/// Create `len` dummy labels
pub fn build_some_labels(len: usize) -> Vec<String> {
    (0..len).into_iter().map(|i| format!("label{i}")).collect()
}

/// Create `len` dummy `Point2D`
pub fn build_some_point2d(len: usize) -> Vec<field_types::Point2D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .into_iter()
        .map(|_| field_types::Point2D {
            x: rng.gen_range(0.0..10.0),
            y: rng.gen_range(0.0..10.0),
        })
        .collect()
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`crate::TimePoint`].
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
    (Timeline::new("log_time", TimeType::Time), log_time.into())
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`crate::TimePoint`].
pub fn build_frame_nr(frame_nr: i64) -> (Timeline, TimeInt) {
    (
        Timeline::new("frame_nr", TimeType::Sequence),
        frame_nr.into(),
    )
}

pub fn build_instances(nb_instances: usize) -> ComponentBundle {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let data = PrimitiveArray::from(
        (0..nb_instances)
            .into_iter()
            .map(|_| Some(rng.gen()))
            .collect::<Vec<Option<u32>>>(),
    );
    let data = wrap_in_listarray(data.boxed());

    ComponentBundle {
        name: "instances".to_owned(),
        component: data.boxed(),
    }
}

pub fn build_rects(nb_instances: usize) -> ComponentBundle {
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
    ComponentBundle {
        name: "rects".to_owned(),
        component: wrap_in_listarray(data.boxed()).boxed(),
    }
}

pub fn build_positions(nb_instances: usize) -> ComponentBundle {
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

    ComponentBundle {
        name: "positions".to_owned(),
        component: data.boxed(),
    }
}
