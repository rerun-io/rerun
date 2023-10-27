//! Log a couple 2D line segments using 2D line strips.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_segments2d").spawn()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log("segments", &rerun::LineStrips2D::new(points.chunks(2)))?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &rerun::Boxes2D::from_centers_and_sizes([(3.0, 0.0)], [(8.0, 6.0)]),
    )?;

    Ok(())
}
