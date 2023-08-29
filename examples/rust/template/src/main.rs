//! Example template.

use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun_example_my_example_name").memory()?;

    let _ = rec_stream;

    // … example code

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
