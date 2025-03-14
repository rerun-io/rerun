//! Sets the recording properties.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    rec.set_properties(
        &rerun::archetypes::RecordingProperties::new()
            .with_started(0)
            .with_name("My recording (initial)"),
    )?;

    // Overwrites the name from above.
    rec.set_name("My recording")?;

    Ok(())
}
