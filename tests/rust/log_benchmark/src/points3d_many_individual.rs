use crate::points3d_shared::{prepare_points3d, Point3DInput};

const NUM_POINTS: usize = 1_000_000;

fn execute(input: Point3DInput) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let Point3DInput {
        positions,
        colors,
        radii,
        label: _,
    } = input;

    let (rec, _storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_benchmark_points3d_many_individual")
            .memory()?;

    for i in 0..NUM_POINTS {
        rec.set_time_sequence("my_timeline", i as i64);
        rec.log(
            "single_point",
            &rerun::Points3D::new([positions[i]])
                .with_colors([colors[i]])
                .with_radii([radii[i]]),
        )?;
    }
    Ok(())
}

/// Log many individual points (position, color, radius), each with a different timestamp.
pub fn run() -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare_points3d(1337, NUM_POINTS));
    execute(input)
}
