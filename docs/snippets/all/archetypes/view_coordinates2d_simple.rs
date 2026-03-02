//! Use the math/plot convention for 2D (Y pointing up).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_view_coordinates2d").spawn()?;

    rec.log_static("world", &rerun::ViewCoordinates2D::RU())?; // Set Y-Up

    rec.log(
        "world/points",
        &rerun::Points2D::new([(0.0, 0.0), (1.0, 1.0), (3.0, 2.0)]),
    )?;

    Ok(())
}
