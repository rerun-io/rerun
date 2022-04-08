//! Serves the web viewer wasm/html

#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]
#![allow(clippy::manual_range_contains)]

use std::task::{Context, Poll};

use futures_util::future;
use hyper::service::Service;
use hyper::{Body, Request, Response};

#[derive(Debug)]
pub struct Svc;

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    #[cfg(feature = "__ci")]
    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        panic!("web_server compiled with '__ci' feature (or `--all-features`). DON'T DO THAT! It's only for the CI!");
    }

    #[cfg(not(feature = "__ci"))]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let rsp = Response::builder();

        let bytes = match req.uri().path() {
            "/" | "/index.html" => &include_bytes!("../../docs/index.html")[..],
            "/favicon.ico" => &include_bytes!("../../docs/favicon.ico")[..],
            "/sw.js" => &include_bytes!("../../docs/sw.js")[..],
            "/viewer_bg.wasm" => &include_bytes!("../../docs/viewer_bg.wasm")[..],
            "/viewer.js" => &include_bytes!("../../docs/viewer.js")[..],
            _ => {
                tracing::warn!("404 path: {}", req.uri().path());
                let body = Body::from(Vec::new());
                let rsp = rsp.status(404).body(body).unwrap();
                return future::ok(rsp);
            }
        };

        let body = Body::from(Vec::from(bytes));
        let rsp = rsp.status(200).body(body).unwrap();
        future::ok(rsp)
    }
}

pub struct MakeSvc;

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _: T) -> Self::Future {
        future::ok(Svc)
    }
}

pub async fn run(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = format!("127.0.0.1:{}", port).parse().unwrap();
    let server = hyper::Server::bind(&bind_addr).serve(MakeSvc);
    println!("Serving viewer on http://{}", bind_addr);
    server.await?;
    Ok(())
}
