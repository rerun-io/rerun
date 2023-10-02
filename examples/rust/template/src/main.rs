//! Example template.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_my_example_name").memory()?;

    let _ = rec;

    // … example code

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
