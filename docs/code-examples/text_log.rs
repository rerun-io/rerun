//! Log a `TextLog`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_text_log").memory()?;

    rec.log(
        "log",
        &rerun::TextLog::new("Application started.").with_level("INFO"),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
