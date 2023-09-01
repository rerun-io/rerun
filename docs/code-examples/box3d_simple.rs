//! Log some a single oriented bounding box
use rerun::components::Box3D;
use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_box3d").memory()?;

    rec.log_component_lists("simple", false, 1, [&Box3D::new(2.0, 2.0, 1.0) as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
