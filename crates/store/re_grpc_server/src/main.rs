use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use re_grpc_server::MessageProxy;
use re_memory::MemoryLimit;
use re_protos::sdk_comms::v0::message_proxy_server::MessageProxyServer;
use tokio::net::TcpListener;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;

const DEFAULT_GRPC_PORT: u16 = 1852;
const DEFAULT_GRPC_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), DEFAULT_GRPC_PORT);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), tonic::transport::Error> {
    re_log::setup_logging();

    let tcp_listener = TcpListener::bind(DEFAULT_GRPC_ADDR)
        .await
        .unwrap_or_else(|err| panic!("failed to bind listener on {DEFAULT_GRPC_ADDR}: {err}"));
    let incoming =
        TcpIncoming::from_listener(tcp_listener, true, None).expect("failed to init listener");

    re_log::info!("Listening for gRPC connections on {DEFAULT_GRPC_ADDR}");

    use tower_http::cors::{Any, CorsLayer};
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let grpc_web = tonic_web::GrpcWebLayer::new();

    let routes = {
        let mut routes_builder = tonic::service::Routes::builder();
        routes_builder.add_service(MessageProxyServer::new(MessageProxy::new(
            MemoryLimit::UNLIMITED,
        )));
        routes_builder.routes()
    };

    Server::builder()
        .accept_http1(true) // Support `grpc-web` clients
        .layer(cors) // Allow CORS requests from web clients
        .layer(grpc_web) // Support `grpc-web` clients
        .add_routes(routes)
        .serve_with_incoming(incoming)
        .await
}
