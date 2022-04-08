#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]
#![allow(clippy::manual_range_contains)]

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    web_server::run(9090).await.unwrap();
}
