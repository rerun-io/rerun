fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a local file handle to stream the data into.
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_native_sync")
        .save("/tmp/my_recording.rrd")?;

    // Log data as usual, thereby writing it into the file.
    loop {
        rec.log("/", &rerun::TextLog::new("Logging things..."))?;
    }
}
