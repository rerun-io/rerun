#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]
#![allow(clippy::manual_range_contains)]

#[tokio::main]
async fn main() {
    re_log::setup_native_logging();
    re_web_server::WebServer::new(9090).serve().await.unwrap();
}
