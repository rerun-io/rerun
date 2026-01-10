use hyper_util::rt::TokioIo;
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::{
    RerunCloudService, RerunCloudServiceServer,
};
use re_redap_client::{ConnectionClient, RedapClient};
use tokio::io::DuplexStream;
use tonic::transport::Channel;
use tonic::transport::server::Connected;

#[derive(Debug)]
pub struct TestIo(pub DuplexStream);

impl Connected for TestIo {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl tokio::io::AsyncRead for TestIo {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for TestIo {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// Utility function to create a [`ConnectionClient`] for a redap test service.
/// Some APIs in our stack require a client. This function creates a simple
/// channel for connecting to a service.
pub async fn create_test_client<S>(service: S) -> ConnectionClient
where
    S: RerunCloudService + Send + 'static,
{
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024); // bigger buffer for real payloads

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(RerunCloudServiceServer::new(service))
            .serve_with_incoming(tokio_stream::once(Ok::<_, std::io::Error>(TestIo(
                server_io,
            ))))
            .await
            .unwrap();
    });

    let mut client_io = Some(TokioIo::new(client_io));
    let channel = Channel::from_static("http://[::]:50051")
        .connect_with_connector(tower::service_fn(move |_: tonic::transport::Uri| {
            let io = client_io.take().expect("connection already established");
            async move { Ok::<_, std::io::Error>(io) }
        }))
        .await
        .expect("failed to connect");

    let middlewares = tower::ServiceBuilder::new()
        .layer(re_auth::client::AuthDecorator::new(None))
        .layer({
            let name = None;
            let version = None;
            let is_client = true;
            re_protos::headers::new_rerun_headers_layer(name, version, is_client)
        });

    #[cfg(feature = "perf_telemetry")]
    let middlewares = middlewares.layer(re_perf_telemetry::new_client_telemetry_layer());

    let svc = tower::ServiceBuilder::new()
        .layer(middlewares.into_inner())
        .service(channel);

    let client: RedapClient = RerunCloudServiceClient::new(svc);

    ConnectionClient::new(client)
}
