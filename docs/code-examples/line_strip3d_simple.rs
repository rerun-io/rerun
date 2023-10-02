//! Log a simple line strip.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_line_strip3d").memory()?;

    let points = [
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];
    rec.log("strip", &rerun::LineStrips3D::new([points]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
