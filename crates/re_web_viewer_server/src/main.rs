#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

#[tokio::main]
async fn main() {
    re_log::setup_native_logging();
    let port = 9090;
    eprintln!("Hosting web-viewer on http://127.0.0.1:{port}");

    // Shutdown server via Ctrl+C
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    ctrlc::set_handler(move || {
        re_log::debug!("Ctrl-C detected - Closing web server.");
        shutdown_tx.send(()).unwrap();
    })
    .expect("Error setting Ctrl-C handler");

    re_web_viewer_server::WebViewerServer::new(port)
        .serve(shutdown_rx)
        .await
        .unwrap();
}
