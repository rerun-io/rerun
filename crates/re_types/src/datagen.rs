//! Generate random data for tests and benchmarks.

// TODO
// TODO(#1810): It really is time for whole module to disappear.

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
