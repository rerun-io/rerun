//! Example template.

use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("minimal_rs").memory()?;

    let _ = rec_stream;

    // ... example code

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
