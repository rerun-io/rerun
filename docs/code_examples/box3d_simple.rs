//! Log a single 3D box.
use rerun::archetypes::Boxes3D;
use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_box3d").memory()?;

    rec.log("simple", &Boxes3D::from_half_sizes([(2.0, 2.0, 1.0)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
