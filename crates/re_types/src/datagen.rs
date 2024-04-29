//! Generate random data for tests and benchmarks.

// TODO
// TODO(#1810): It really is time for whole module to disappear.

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
