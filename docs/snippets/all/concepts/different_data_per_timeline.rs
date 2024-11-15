//! Log different data on different timelines.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_different_data_per_timeline").spawn()?;

    rec.set_time_sequence("blue timeline", 0);
    rec.set_time_seconds("red timeline", 0.0);
    rec.log("points", &rerun::Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

    // Log a red color on one timeline.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_time_seconds("red timeline", 1.0);
    rec.log(
        "points",
        &[&rerun::components::Color::from_u32(0xFF0000FF) as &dyn rerun::ComponentBatch],
    )?;

    // And a blue color on the other.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_time_sequence("blue timeline", 1);
    rec.log(
        "points",
        &[&rerun::components::Color::from_u32(0x0000FFFF) as &dyn rerun::ComponentBatch],
    )?;

    // TODO(#5521): log VisualBounds2D

    Ok(())
}
