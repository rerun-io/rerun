//! Create and set a file sink.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_file_sink").memory()?;

    rec.set_sink(rerun::FileSink::new("recording.rrd"));

    Ok(())
}
