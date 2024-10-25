fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the Rerun TCP server using the default address and
    // port: localhost:9876
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_native_sync").connect()?;

    // Log data as usual, thereby pushing it into the TCP socket.
    loop {
        rec.log("/", &rerun::TextLog::new("Logging things..."))?;
    }
}
