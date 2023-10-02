//! Log a `TextLog`

use rerun::{archetypes::TextLog, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_text_log").memory()?;

    rec.log(
        "log",
        &TextLog::new("Application started.").with_level("INFO"),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
