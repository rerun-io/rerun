//! Create and set a GRPC sink.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_grpc_sink").buffered()?;

    // The default URL is `rerun+http://127.0.0.1:9876/proxy`
    // This can be used to connect to a viewer on a different machine
    rec.set_sink(Box::new(rerun::sink::GrpcSink::new(
        "rerun+http://127.0.0.1:9876/proxy".parse()?,
    )));

    Ok(())
}
