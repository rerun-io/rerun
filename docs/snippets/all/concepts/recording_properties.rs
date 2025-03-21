//! Sets the recording properties.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    // Recordings can have an optional name.
    rec.send_recording_name("My recording")?;

    // Start time is set automatically, but we can overwrite it at any time.
    rec.send_recording_start_time(1742539110661000000)?;

    // Adds a user-defined property to the recording.
    rec.send_property(
        "camera_left",
        &rerun::archetypes::Points3D::new([[1.0, 0.1, 1.0]]),
    )?;

    // Adds another property, this time with user-defined data.
    rec.send_property("tags", &rerun::archetypes::Points3D::new([[0.1, 1.0, 0.1]]))?;

    // Properties, including the name, can be overwritten at any time.
    rec.send_recording_name("My episode")?;

    Ok(())
}
