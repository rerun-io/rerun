//! Integration tests for rerun and the in memory server.

mod test_data;

use re_grpc_client::{ConnectionClient, ConnectionError, ConnectionRegistry};
use re_server::{FrontendHandlerBuilder, ServerBuilder, ServerHandle};
use re_uri::external::url::Host;
use std::net::{SocketAddr, TcpListener};

pub struct TestServer {
    _server_handle: ServerHandle,
    port: u16,
}

impl TestServer {
    pub async fn spawn() -> Self {
        // Get a random free port
        let port = get_free_port();

        println!("Spawning server on port {port}");

        let frontend_server = {
            use re_protos::frontend::v1alpha1::frontend_service_server::FrontendServiceServer;
            let builder = FrontendHandlerBuilder::new();
            FrontendServiceServer::new(builder.build())
                .max_decoding_message_size(re_grpc_server::MAX_DECODING_MESSAGE_SIZE)
                .max_encoding_message_size(re_grpc_server::MAX_ENCODING_MESSAGE_SIZE)
        };

        let server_builder = ServerBuilder::default()
            .with_address(SocketAddr::from(([0, 0, 0, 0], port)))
            .with_service(frontend_server);

        let server = server_builder.build();
        let mut server_handle = server.start();

        server_handle
            .wait_for_ready()
            .await
            .expect("Can't start server");

        println!("Server ready on port {port}");

        Self {
            _server_handle: server_handle,
            port,
        }
    }

    pub async fn with_test_data(self) -> Self {
        self.add_test_data().await;
        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn client(&self) -> Result<ConnectionClient, ConnectionError> {
        let origin = re_uri::Origin {
            host: Host::Domain("localhost".to_owned()),
            port: self.port,
            scheme: re_uri::Scheme::RerunHttp,
        };
        ConnectionRegistry::new().client(origin).await
    }

    pub async fn add_test_data(&self) {
        let client = self.client().await.expect("Failed to connect");
        test_data::load_test_data(client)
            .await
            .expect("Failed to load test data");
    }
}

/// Get a free port from the OS.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    addr.port()
}
