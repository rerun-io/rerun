//! Log some very simple points.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_simple").memory()?;

    rec.log(
        "points",
        &rerun::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
