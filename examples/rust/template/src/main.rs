//! Example template.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_example_name").spawn(rerun::default_flush_timeout())?;

    // … example code
    _ = rec;

    Ok(())
}
