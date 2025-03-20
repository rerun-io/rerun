//! Sets the recording properties.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    // Overwrites the name from above.
    rec.send_recording_name("My recording")?;

    // Overwrites the start time from above.
    rec.send_recording_start_time(42)?;

    // Adds a user-defined property to the recording at
    rec.send_property(
        "camera_left",
        &rerun::archetypes::Points3D::new([[1.0, 0.1, 1.0]]),
    )?;

    Ok(())
}
