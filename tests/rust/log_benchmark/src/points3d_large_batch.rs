use crate::points3d_shared::{Point3DInput, prepare_points3d};

const NUM_POINTS: usize = 50_000_000;

fn execute(input: Point3DInput) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let Point3DInput {
        positions,
        colors,
        radii,
        label,
    } = input;

    let (rec, _storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_benchmark_points3d_large_batch")
            .memory()?;
    rec.log(
        "large_batch",
        &rerun::Points3D::new(positions)
            .with_colors(colors)
            .with_radii(radii)
            .with_labels([label]),
    )?;
    Ok(())
}

/// Log a single large batch of points with positions, colors, radii and a splatted string.
pub fn run() -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare_points3d(42, NUM_POINTS));
    execute(input)
}
