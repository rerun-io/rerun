use re_grpc_server::MessageProxy;
use re_memory::MemoryLimit;
use re_protos::sdk_comms::v0::message_proxy_server::MessageProxyServer;
use tokio::net::TcpListener;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), tonic::transport::Error> {
    re_log::setup_logging();

    let tcp_listener = TcpListener::bind("127.0.0.1:1852")
        .await
        .expect("failed to bind listener on 127.0.0.1:1852");
    let incoming =
        TcpIncoming::from_listener(tcp_listener, true, None).expect("failed to init listener");

    re_log::info!("Listening for gRPC connections on 127.0.0.1:1852");

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
