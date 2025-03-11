//! Sets the recording properties.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    rec.set_recording_name("My recording");

    Ok(())
}
