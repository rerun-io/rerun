#![expect(clippy::let_underscore_untyped)]
#![expect(clippy::let_underscore_must_use)]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _};

use tokio::net::TcpListener;
use tokio::sync::oneshot::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt as _;
use tonic::service::{Routes, RoutesBuilder};
use tracing::{error, info};

// ---

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("Ready channel closed unexpectedly")]
    ReadyChannelClosedUnexpectedly,

    #[error("Failed channel closed unexpectedly")]
    FailedChannelClosedUnexpectedly,

    #[error("Server failed to start: {reason}")]
    ServerFailedToStart { reason: String },
}

/// An instance of a Redap gRPC server.
///
/// Use [`ServerBuilder`] to create a new instance.
pub struct Server {
    addr: SocketAddr,
    routes: Routes,
    artificial_latency: std::time::Duration,
}

/// `ServerHandle` is a tiny helper abstraction that enables us to
/// deal with the gRPC server lifecycle more easily.
pub struct ServerHandle {
    shutdown: Option<Sender<()>>,
    ready: mpsc::Receiver<SocketAddr>,
    failed: mpsc::Receiver<String>,
}

impl ServerHandle {
    /// Wait until the server is ready to accept connections (or failure occurs)
    pub async fn wait_for_ready(&mut self) -> Result<SocketAddr, ServerError> {
        tokio::select! {
            ready = self.ready.recv() => {
                match ready {
                    Some(local_addr) => {
                        info!("Ready for connections.");
                        Ok(local_addr)
                    },
                    None => Err(ServerError::ReadyChannelClosedUnexpectedly)

                }
            }
            failed = self.failed.recv() => {
                match failed {
                    Some(reason) => Err(ServerError::ServerFailedToStart { reason }),
                    None => Err(ServerError::FailedChannelClosedUnexpectedly)

                }
            }
        }
    }

    /// Wait until the server is shutdown.
    pub async fn wait_for_shutdown(&mut self) {
        self.failed.recv().await;
    }

    /// Signal to the gRPC server to shutdown, and then wait for it.
    pub async fn shutdown_and_wait(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.send(()).ok();
            self.wait_for_shutdown().await;
        }
    }
}

impl Server {
    /// Starts the server and return `ServerHandle` so that caller can manage
    /// the server lifecycle.
    pub fn start(self) -> ServerHandle {
        let Self {
            addr,
            routes,
            artificial_latency,
        } = self;

        let (ready_tx, ready_rx) = mpsc::channel(1);
        let (failed_tx, failed_rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let listener = if let Ok(listener) = TcpListener::bind(addr).await {
                #[expect(clippy::unwrap_used)]
                let bind_addr = listener.local_addr().unwrap();

                let mut connect_addr = bind_addr;

                if connect_addr.ip().is_unspecified() {
                    // We usually cannot connect to "0.0.0.0" so we swap it for 127.0.0.1:
                    if connect_addr.is_ipv4() {
                        connect_addr.set_ip(Ipv4Addr::LOCALHOST.into());
                    } else {
                        connect_addr.set_ip(Ipv6Addr::LOCALHOST.into());
                    }
                }

                info!(
                    "Listening on {bind_addr}. To connect the Rerun Viewer, use the following address: rerun+http://{connect_addr}"
                );

                #[expect(clippy::unwrap_used)]
                ready_tx.send(connect_addr).await.unwrap();
                listener
            } else {
                error!("Failed to bind to address {addr}");
                #[expect(clippy::unwrap_used)]
                failed_tx
                    .send(format!("Failed to bind to address {addr}"))
                    .await
                    .unwrap();
                return;
            };

            // NOTE: We already set NODELAY at the `tonic` layer just below, but that might
            // or might not be good enough depending on a bunch of external conditions: make
            // sure to disable Nagle's on every socket as soon as they're accepted, no
            // matter what.
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener).map(|inc| {
                let inc = inc?;
                inc.set_nodelay(true)?;
                Ok::<_, std::io::Error>(inc)
            });

            let middlewares = tower::ServiceBuilder::new()
                .layer({
                    let name = Some("rerun-oss".to_owned());
                    let version = None;
                    let is_client = false;
                    re_protos::headers::new_rerun_headers_layer(name, version, is_client)
                })
                .layer(tower_http::cors::CorsLayer::permissive()) // Allow CORS for all origins (to support web clients)
                .layer(crate::latency_layer::LatencyLayer::new(artificial_latency))
                // NOTE: GrpcWebLayer is applied directly to gRPC routes in ServerBuilder::build()
                // to avoid rejecting regular HTTP requests
                .into_inner();

            let mut builder = tonic::transport::Server::builder()
                // NOTE: This NODELAY very likely does nothing because of the call to
                // `serve_with_incoming_shutdown` below, but we better be on the defensive here so
                // we don't get surprised when things inevitably change.
                .tcp_nodelay(true)
                .accept_http1(true)
                .http2_adaptive_window(Some(true)) // Optimize for high throughput
                .layer(middlewares);

            let _ = builder
                .add_routes(routes)
                .serve_with_incoming_shutdown(incoming, async {
                    shutdown_rx.await.ok();
                })
                .await
                .map_err(|err| {
                    error!("Server error: {err:#}");
                    err
                });

            let _ = failed_tx.send("gRPC server stopped".to_owned()).await;
        });

        ServerHandle {
            shutdown: Some(shutdown_tx),
            ready: ready_rx,
            failed: failed_rx,
        }
    }
}

const DEFAULT_ADDRESS: &str = "127.0.0.1:51234";

/// Builder for the gRPC server instance.
#[derive(Default)]
pub struct ServerBuilder {
    addr: Option<SocketAddr>,
    routes_builder: RoutesBuilder,
    axum_routes: axum::Router,
    artificial_latency: std::time::Duration,
}

impl ServerBuilder {
    #[inline]
    pub fn with_address(mut self, addr: SocketAddr) -> Self {
        self.addr = Some(addr);
        self
    }

    pub fn with_service<S>(mut self, svc: S) -> Self
    where
        S: tower_service::Service<
                http::Request<tonic::body::Body>,
                Response = http::Response<tonic::body::Body>,
                Error = std::convert::Infallible,
            > + tonic::server::NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    {
        self.routes_builder.add_service(svc);
        self
    }

    pub fn with_http_route(mut self, path: &str, handler: axum::routing::MethodRouter) -> Self {
        self.axum_routes = self.axum_routes.route(path, handler);
        self
    }

    /// Add fake latency to simulate a remote server.
    pub fn with_artificial_latency(mut self, artificial_latency: std::time::Duration) -> Self {
        self.artificial_latency = artificial_latency;
        self
    }

    pub fn build(self) -> Server {
        let Self {
            addr,
            routes_builder,
            axum_routes,
            artificial_latency,
        } = self;

        let grpc_routes = routes_builder.routes();
        let grpc_routes = grpc_routes.into_axum_router();

        // Apply GrpcWebLayer only to gRPC routes, not HTTP routes
        let grpc_routes = grpc_routes.layer(tonic_web::GrpcWebLayer::new());

        let routes =
            grpc_routes
                .merge(axum_routes)
                .fallback(|_req: axum::extract::Request| async {
                    use axum::response::IntoResponse as _;
                    http::StatusCode::NOT_FOUND.into_response()
                });

        Server {
            #[expect(clippy::unwrap_used)]
            addr: addr
                .unwrap_or_else(|| DEFAULT_ADDRESS.to_socket_addrs().unwrap().next().unwrap()),
            routes: routes.into(),
            artificial_latency,
        }
    }
}
