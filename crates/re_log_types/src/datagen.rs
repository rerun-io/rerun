//! Generate random data for tests and benchmarks.

use crate::{
    field_types::{self, Instance},
    msg_bundle::{wrap_in_listarray, Component as _, ComponentBundle},
    Time, TimeInt, TimeType, Timeline,
};
use arrow2::array::PrimitiveArray;

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

//TODO(john) convert this to a Component struct
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
        name: Instance::NAME.to_owned(),
        value: data.boxed(),
    }
}
