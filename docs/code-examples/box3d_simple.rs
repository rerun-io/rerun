//! Log a single 3D box.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_box3d").memory()?;

    rec.log(
        "simple",
        &rerun::Boxes3D::from_half_sizes([(2.0, 2.0, 1.0)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
