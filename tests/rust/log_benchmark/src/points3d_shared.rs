use crate::lcg;

/// Batch that should be logged.
/// Intentionally not using `rerun::Points3D` here already.
pub struct Point3DInput {
    pub positions: Vec<glam::Vec3>,
    pub colors: Vec<u32>,
    pub radii: Vec<f32>,
    pub label: String,
}

pub fn prepare_points3d(mut lcg_state: i64, num_points: usize) -> Point3DInput {
    re_tracing::profile_function!();

    Point3DInput {
        positions: (0..num_points)
            .map(|_| {
                glam::vec3(
                    lcg(&mut lcg_state) as f32,
                    lcg(&mut lcg_state) as f32,
                    lcg(&mut lcg_state) as f32,
                )
            })
            .collect(),
        colors: (0..num_points)
            .map(|_| lcg(&mut lcg_state) as u32)
            .collect(),
        radii: (0..num_points)
            .map(|_| lcg(&mut lcg_state) as f32)
            .collect(),
        label: "large_batch".to_owned(),
    }
}
