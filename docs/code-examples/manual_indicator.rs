//! Shows how to manually associate one or more indicator components with arbitrary data.

use rerun::{
    archetypes::{Mesh3D, Points3D},
    components::{Color, Position3D, Radius},
    Archetype, ComponentBatch, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_manual_indicator").memory()?;

    // Specify both a Mesh3D and a Points3D indicator component so that the data is shown as both a
    // 3D mesh _and_ a point cloud by default.
    rec.log_component_batches(
        "points_and_mesh",
        false,
        [
            Points3D::indicator().as_ref() as &dyn ComponentBatch,
            Mesh3D::indicator().as_ref(),
            &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]].map(Position3D::from),
            &[[255, 0, 0], [0, 255, 0], [0, 0, 255]].map(|[r, g, b]| Color::from_rgb(r, g, b)),
            &[1.0].map(Radius::from),
        ],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
