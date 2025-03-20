//! Sets the recording properties.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    rec.set_properties(
        &rerun::archetypes::RecordingProperties::new()
            .with_start_time(0)
            .with_name("My recording (initial)"),
    )?;

    // Overwrites the name from above.
    rec.set_recording_name("My recording")?;

    // Overwrites the start time from above.
    rec.set_recording_start_time(42)?;

    // Adds a user-defined property to the recording at
    rec.set_properties_with_prefix(
        "cameras/left",
        &rerun::archetypes::Points3D::new([[1.0, 0.1, 1.0]]),
    )?;

    Ok(())
}
