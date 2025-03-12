//! Log different data on different timelines.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_different_data_per_timeline").spawn()?;

    rec.set_time_sequence("blue timeline", 0);
    rec.set_duration_seconds("red timeline", 0.0);
    rec.log("points", &rerun::Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

    // Log a red color on one timeline.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_duration_seconds("red timeline", 1.0);
    rec.log(
        "points",
        &rerun::Points2D::update_fields().with_colors([0xFF0000FF]),
    )?;

    // And a blue color on the other.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_time_sequence("blue timeline", 1);
    rec.log(
        "points",
        &rerun::Points2D::update_fields().with_colors([0x0000FFFF]),
    )?;

    // TODO(#5521): log VisualBounds2D

    Ok(())
}
