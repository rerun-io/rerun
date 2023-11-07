use crate::lcg;

const NUM_POINTS: usize = 50_000_000;

/// Log a single large batch of points with positions, colors, radii and a splatted string.
pub fn run() -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare());
    execute(input)
}

/// Batch that should be logged.
/// Intentionally not using `rerun::Points3D` here already.
struct PointBatchInput {
    positions: Vec<glam::Vec3>,
    colors: Vec<u32>,
    radii: Vec<f32>,
    label: String,
}

fn prepare() -> PointBatchInput {
    re_tracing::profile_function!();
    let mut lcg_state = 42_i64;

    PointBatchInput {
        positions: (0..NUM_POINTS)
            .map(|_| {
                glam::vec3(
                    lcg(&mut lcg_state) as f32,
                    lcg(&mut lcg_state) as f32,
                    lcg(&mut lcg_state) as f32,
                )
            })
            .collect(),
        colors: (0..NUM_POINTS)
            .map(|_| lcg(&mut lcg_state) as u32)
            .collect(),
        radii: (0..NUM_POINTS)
            .map(|_| lcg(&mut lcg_state) as f32)
            .collect(),
        label: "large_batch".to_owned(),
    }
}

fn execute(input: PointBatchInput) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let PointBatchInput {
        positions,
        colors,
        radii,
        label,
    } = input;

    let (rec, _storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_random").memory()?;
    rec.log(
        "large_batch",
        &rerun::Points3D::new(positions)
            .with_colors(colors)
            .with_radii(radii)
            .with_labels([label]),
    )?;
    Ok(())
}
