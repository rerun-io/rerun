//! Log a `TextLog`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_log")
        .spawn(&rerun::SpawnOptions::default(), None)?;

    rec.log(
        "log",
        &rerun::TextLog::new("Application started.").with_level("INFO"),
    )?;

    Ok(())
}
