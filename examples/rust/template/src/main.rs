//! Example template.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_example_name")
        .spawn(&rerun::SpawnOptions::default(), None)?;

    // … example code
    _ = rec;

    Ok(())
}
