#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]
#![allow(clippy::manual_range_contains)]

#[tokio::main]
async fn main() {
    re_log::setup_native_logging();
    let port = 9090;
    eprintln!("Hosting web-viewer on http://127.0.0.1:{port}");
    re_web_viewer_server::WebServer::new(port)
        .serve()
        .await
        .unwrap();
}
