fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the Rerun gRPC server using the default address and
    // port: localhost:9876
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_log_to_grpc").connect_grpc()?;

    // Log data as usual, thereby pushing it into the stream.
    loop {
        rec.log("/", &rerun::TextLog::new("Logging things…"))?;
    }
}
