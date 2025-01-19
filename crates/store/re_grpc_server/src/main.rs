use re_grpc_server::MessageProxy;
use re_memory::MemoryLimit;
use re_protos::sdk_comms::v0::message_proxy_server::MessageProxyServer;
use tokio::net::TcpListener;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), tonic::transport::Error> {
    let tcp_listener = TcpListener::bind("127.0.0.1:1852")
        .await
        .expect("failed to bind listener on 127.0.0.1:1852");
    let incoming =
        TcpIncoming::from_listener(tcp_listener, true, None).expect("failed to init listener");

    Server::builder()
        .add_service(MessageProxyServer::new(MessageProxy::new(
            MemoryLimit::UNLIMITED,
        )))
        .serve_with_incoming(incoming)
        .await
}
