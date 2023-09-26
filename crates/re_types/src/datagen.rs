//! Generate random data for tests and benchmarks.

// TODO(#1810): It really is time for whole module to disappear.

use crate::components::InstanceKey;

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Vec<crate::components::Color> {
    (0..len)
        .map(|i| crate::components::Color::from(i as u32))
        .collect()
}

/// Create `len` dummy `Position2D`
pub fn build_some_positions2d(len: usize) -> Vec<crate::components::Position2D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| {
            crate::components::Position2D::new(rng.gen_range(0.0..10.0), rng.gen_range(0.0..10.0))
        })
        .collect()
}

/// Create `len` dummy `Vec3D`
pub fn build_some_vec3d(len: usize) -> Vec<crate::datatypes::Vec3D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| {
            crate::datatypes::Vec3D::new(
                rng.gen_range(0.0..10.0),
                rng.gen_range(0.0..10.0),
                rng.gen_range(0.0..10.0),
            )
        })
        .collect()
}

/// Create `len` dummy `InstanceKey` keys. These keys will be sorted.
pub fn build_some_instances(num_instances: usize) -> Vec<InstanceKey> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    // Allocate pool of 10x the potential instance keys, draw a random sampling, and then sort it
    let mut instance_pool = (0..(num_instances * 10)).collect::<Vec<_>>();
    let (rand_instances, _) = instance_pool.partial_shuffle(&mut rng, num_instances);
    let mut sorted_instances = rand_instances.to_vec();
    sorted_instances.sort();

    sorted_instances
        .into_iter()
        .map(|id| InstanceKey(id as u64))
        .collect()
}

pub fn build_some_instances_from(instances: impl IntoIterator<Item = u64>) -> Vec<InstanceKey> {
    let mut instances = instances.into_iter().map(InstanceKey).collect::<Vec<_>>();
    instances.sort();
    instances
}
