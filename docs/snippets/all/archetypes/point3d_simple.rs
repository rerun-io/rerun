//! Log some very simple points.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d")
        .save("/tmp/points3d_simple.rrd")?;

    rec.log(
        "points",
        &rerun::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
    )?;

    Ok(())
}
