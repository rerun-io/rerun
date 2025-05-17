#![expect(clippy::let_underscore_untyped)]
#![expect(clippy::let_underscore_must_use)]

use std::net::SocketAddr;
use std::net::ToSocketAddrs as _;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;
use tokio_stream::StreamExt as _;
use tonic::service::Routes;
use tonic::service::RoutesBuilder;
use tracing::error;
use tracing::info;

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
}

/// `ServerHandle` is a tiny helper abstraction that enables us to
/// deal with the gRPC server lifecycle more easily.
pub struct ServerHandle {
    shutdown: Sender<()>,
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
                        info!("Ready for connections");
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

    /// Signal to the gRPC server to shutdown.
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

impl Server {
    /// Starts the server and return `ServerHandle` so that caller can manage
    /// the server lifecycle.
    pub fn start(self) -> ServerHandle {
        let (ready_tx, ready_rx) = mpsc::channel(1);
        let (failed_tx, failed_rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let listener = if let Ok(listener) = TcpListener::bind(self.addr).await {
                #[expect(clippy::unwrap_used)]
                let local_addr = listener.local_addr().unwrap();
                info!("Listening on {local_addr}");

                #[expect(clippy::unwrap_used)]
                ready_tx.send(local_addr).await.unwrap();
                listener
            } else {
                error!("Failed to bind to address {}", self.addr);
                #[expect(clippy::unwrap_used)]
                failed_tx
                    .send(format!("Failed to bind to address {}", self.addr))
                    .await
                    .unwrap();
                return;
            };

            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener).map(|inc| {
                inc.and_then(|inc| {
                    // NOTE: We already set NODELAY at the `tonic` layer just below, but that might
                    // or might not be good enough depending on a bunch of external conditions: make
                    // sure to disable Nagle's on every socket as soon as they're accepted, no
                    // matter what.
                    inc.set_nodelay(true)?;
                    Ok(inc)
                })
            });

            let middlewares = tower::ServiceBuilder::new()
                .layer(tonic_web::GrpcWebLayer::new()) // Support `grpc-web` clients
                .into_inner();

            let mut builder = tonic::transport::Server::builder()
                // NOTE: This NODELAY very likely does nothing because of the call to
                // `serve_with_incoming_shutdown` below, but we better be on the defensive here so
                // we don't get surprised when things inevitably change.
                .tcp_nodelay(true)
                .accept_http1(true)
                .layer(middlewares);

            let _ = builder
                .add_routes(self.routes)
                .serve_with_incoming_shutdown(incoming, async {
                    shutdown_rx.await.ok();
                })
                .await
                .map_err(|err| {
                    error!("Server error: {:?}", err);
                    err
                });

            let _ = failed_tx.send("gRPC server stopped".to_owned()).await;
        });

        ServerHandle {
            shutdown: shutdown_tx,
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
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<tonic::body::BoxBody>,
                Error = std::convert::Infallible,
            > + tonic::server::NamedService
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    {
        self.routes_builder.add_service(svc);
        self
    }

    pub fn build(self) -> Server {
        Server {
            #[expect(clippy::unwrap_used)]
            addr: self
                .addr
                .unwrap_or(DEFAULT_ADDRESS.to_socket_addrs().unwrap().next().unwrap()),
            routes: self.routes_builder.routes(),
        }
    }
}
