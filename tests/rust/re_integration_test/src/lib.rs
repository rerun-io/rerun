//! Integration tests for rerun and the in memory server.

mod test_data;

use re_grpc_client::{ConnectionClient, ConnectionError, ConnectionRegistry};
use re_server::ServerHandle;
use re_uri::external::url::Host;
use std::net::TcpListener;

pub struct TestServer {
    _server_handle: ServerHandle,
    port: u16,
}

impl TestServer {
    pub async fn spawn() -> Self {
        // Get a random free port
        let port = get_free_port();

        let args = re_server::Args {
            addr: "127.0.0.1".to_owned(),
            port,
            datasets: vec![],
        };
        let server_handle = args
            .create_server_handle()
            .await
            .expect("Can't create server");

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
