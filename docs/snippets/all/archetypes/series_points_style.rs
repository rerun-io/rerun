//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_series_point_style").spawn()?;

    // Set up plot styling:
    // They are logged static as they don't change over time and apply to all timelines.
    // Log two point series under a shared root so that they show in the same plot by default.
    rec.log_static(
        "trig/sin",
        &rerun::SeriesPoints::new()
            .with_colors([[255, 0, 0]])
            .with_names(["sin(0.01t)"])
            .with_markers([rerun::components::MarkerShape::Circle])
            .with_marker_sizes([4.0]),
    )?;
    rec.log_static(
        "trig/cos",
        &rerun::SeriesPoints::new()
            .with_colors([[0, 255, 0]])
            .with_names(["cos(0.01t)"])
            .with_markers([rerun::components::MarkerShape::Cross])
            .with_marker_sizes([2.0]),
    )?;

    for t in 0..((std::f32::consts::TAU * 2.0 * 10.0) as i64) {
        rec.set_time_sequence("step", t);

        // Log two time series under a shared root so that they show in the same plot by default.
        rec.log("trig/sin", &rerun::Scalars::one((t as f64 / 10.0).sin()))?;
        rec.log("trig/cos", &rerun::Scalars::one((t as f64 / 10.0).cos()))?;
    }

    Ok(())
}
