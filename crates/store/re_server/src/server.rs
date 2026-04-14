#![expect(clippy::let_underscore_untyped)]
#![expect(clippy::let_underscore_must_use)]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _};

use tokio::net::TcpListener;
use tokio::sync::oneshot::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt as _;
use tonic::service::{Routes, RoutesBuilder};
use tracing::{error, info};

use crate::error_layer::InjectedErrors;

// ---

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
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
    bandwidth_limit: Option<u64>,
    cors_allowed_origins: Vec<String>,
}

/// `ServerHandle` is a tiny helper abstraction that enables us to
/// deal with the gRPC server lifecycle more easily.
pub struct ServerHandle {
    shutdown: Option<Sender<()>>,
    failed: mpsc::Receiver<String>,
    _task: tokio::task::JoinHandle<()>,

    /// The address clients should connect to.
    connect_addr: SocketAddr,

    /// Test hook: endpoints registered here will return an error.
    injected_errors: InjectedErrors,
}

impl ServerHandle {
    /// The address clients should use to connect.
    ///
    /// This is a connectable address, e.g. `127.0.0.1:9876` instead of `0.0.0.0:9876`.
    pub fn connect_addr(&self) -> SocketAddr {
        self.connect_addr
    }

    /// For testing: get a reference to the injected errors, which can be used to make specific gRPC endpoints fail.
    pub fn injected_errors(&self) -> &InjectedErrors {
        &self.injected_errors
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
    /// Starts the server, waits for it to be ready, and returns a [`ServerHandle`].
    pub async fn start(self) -> Result<ServerHandle, ServerError> {
        let Self {
            addr,
            routes,
            artificial_latency,
            bandwidth_limit,
            cors_allowed_origins,
        } = self;

        let (ready_tx, mut ready_rx) = mpsc::channel(1);
        let (failed_tx, failed_rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let injected_errors = InjectedErrors::new();

        let injected_errors_for_handle = injected_errors.clone();
        let task = tokio::spawn(async move {
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
                .layer(re_grpc_server::cors_layer(&cors_allowed_origins))
                .layer(crate::latency_layer::LatencyLayer::new(artificial_latency))
                .layer(crate::bandwidth_layer::BandwidthLayer::new(bandwidth_limit))
                .layer(re_protos::trace_id_layer::TraceIdLayer::new(
                    std::sync::Arc::new(|| {
                        // We inject a dummy trace-id here so that our e2e integration tests
                        // can verify that the trace-id shows up in error messages.
                        // We sometimes run these tests on release builds, so we always inject these trace-ids.
                        const DUMMY_TRACE_ID: u128 = 0xabba000000000000000000000000abba_u128;
                        Some(opentelemetry::TraceId::from(DUMMY_TRACE_ID))
                    }),
                ))
                .layer(crate::error_layer::ErrorInjectionLayer::new(
                    injected_errors.clone(),
                ))
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

        // Wait for the server to signal readiness.
        let mut failed_rx_for_select = failed_rx;
        let connect_addr = tokio::select! {
            ready = ready_rx.recv() => {
                match ready {
                    Some(addr) => {
                        info!("Ready for connections.");
                        Ok(addr)
                    },
                    None => Err(ServerError::ServerFailedToStart {
                        reason: "ready channel closed unexpectedly".into(),
                    }),
                }
            }
            failed = failed_rx_for_select.recv() => {
                match failed {
                    Some(reason) => Err(ServerError::ServerFailedToStart { reason }),
                    None => Err(ServerError::ServerFailedToStart {
                        reason: "failed channel closed unexpectedly".into(),
                    }),
                }
            }
        }?;

        Ok(ServerHandle {
            shutdown: Some(shutdown_tx),
            failed: failed_rx_for_select,
            _task: task,
            connect_addr,
            injected_errors: injected_errors_for_handle,
        })
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
    bandwidth_limit: Option<u64>,
    cors_allowed_origins: Vec<String>,
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

    /// Limit response bandwidth in bytes per second.
    pub fn with_bandwidth_limit(mut self, bytes_per_second: Option<u64>) -> Self {
        self.bandwidth_limit = bytes_per_second;
        self
    }

    /// Set additional origin patterns allowed to make cross-origin requests.
    ///
    /// By default, only `localhost`, `127.0.0.1`, and `rerun.io` are allowed.
    pub fn with_cors_allowed_origins(mut self, origins: Vec<String>) -> Self {
        self.cors_allowed_origins = origins;
        self
    }

    pub fn build(self) -> Server {
        let Self {
            addr,
            routes_builder,
            axum_routes,
            artificial_latency,
            bandwidth_limit,
            cors_allowed_origins,
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
            bandwidth_limit,
            cors_allowed_origins,
        }
    }
}
