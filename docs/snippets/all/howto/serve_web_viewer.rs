//! Demonstrates how to log data to a gRPC server and connect the web viewer to it.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start a gRPC server and use it as log sink.
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_serve_web_viewer").serve_grpc()?;

    // Connect the web viewer to the gRPC server and open it in the browser.
    let _server_guard = rerun::serve_web_viewer(rerun::web_viewer::WebViewerConfig {
        connect_to: Some("rerun+http://localhost/proxy".to_owned()),
        ..Default::default()
    })?;

    // Log some data to the gRPC server.
    rec.log("data", &rerun::Boxes3D::from_half_sizes([(2.0, 2.0, 1.0)]))?;

    // Keep server running. If we cancel it too early, data may never arrive in the browser.
    std::thread::sleep(std::time::Duration::from_secs(u64::MAX));

    Ok(())
}
