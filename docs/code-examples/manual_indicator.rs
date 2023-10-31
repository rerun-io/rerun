//! Shows how to manually associate one or more indicator components with arbitrary data.

use rerun::Archetype as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_manual_indicator").spawn()?;

    // Specify both a Mesh3D and a Points3D indicator component so that the data is shown as both a
    // 3D mesh _and_ a point cloud by default.
    rec.log_component_batches(
        "points_and_mesh",
        false,
        [
            rerun::Points3D::indicator().as_ref() as &dyn rerun::ComponentBatch,
            rerun::Mesh3D::indicator().as_ref(),
            &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]].map(rerun::Position3D::from),
            &[[255, 0, 0], [0, 255, 0], [0, 0, 255]]
                .map(|[r, g, b]| rerun::Color::from_rgb(r, g, b)),
            &[1.0].map(rerun::Radius::from),
        ],
    )?;

    Ok(())
}
