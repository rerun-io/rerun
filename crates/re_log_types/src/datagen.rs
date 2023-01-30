//! Generate random data for tests and benchmarks.

use crate::{
    component_types::{self, Instance},
    Time, TimeInt, TimeType, Timeline,
};

/// Create `len` dummy rectangles
pub fn build_some_rects(len: usize) -> Vec<component_types::Rect2D> {
    (0..len)
        .into_iter()
        .map(|i| {
            component_types::Rect2D::from_xywh(i as f32, i as f32, (i / 2) as f32, (i / 2) as f32)
        })
        .collect()
}

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Vec<component_types::ColorRGBA> {
    (0..len)
        .into_iter()
        .map(|i| component_types::ColorRGBA(i as u32))
        .collect()
}

/// Create `len` dummy labels
pub fn build_some_labels(len: usize) -> Vec<String> {
    (0..len).into_iter().map(|i| format!("label{i}")).collect()
}

/// Create `len` dummy `Point2D`
pub fn build_some_point2d(len: usize) -> Vec<component_types::Point2D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .into_iter()
        .map(|_| component_types::Point2D {
            x: rng.gen_range(0.0..10.0),
            y: rng.gen_range(0.0..10.0),
        })
        .collect()
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`crate::TimePoint`].
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
    (Timeline::log_time(), log_time.into())
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`crate::TimePoint`].
pub fn build_frame_nr(frame_nr: TimeInt) -> (Timeline, TimeInt) {
    (Timeline::new("frame_nr", TimeType::Sequence), frame_nr)
}

/// Create `len` dummy `Instance` keys. These keys will be sorted.
pub fn build_some_instances(nb_instances: usize) -> Vec<Instance> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    // Allocate pool of 10x the potential instance keys, draw a random sampling, and then sort it
    let mut instance_pool = (0..(nb_instances * 10)).collect::<Vec<_>>();
    let (rand_instances, _) = instance_pool.partial_shuffle(&mut rng, nb_instances);
    let mut sorted_instances = rand_instances.to_vec();
    sorted_instances.sort();

    sorted_instances
        .into_iter()
        .map(|id| Instance(id as u64))
        .collect()
}

pub fn build_some_instances_from(instances: impl IntoIterator<Item = u64>) -> Vec<Instance> {
    let mut instances = instances.into_iter().map(Instance).collect::<Vec<_>>();
    instances.sort();
    instances
}
