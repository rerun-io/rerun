//! Integration tests for rerun and the in memory server.

mod kittest_harness_ext;
mod test_data;
mod viewer_section;

use std::net::TcpListener;

pub use kittest_harness_ext::HarnessExt;
use re_protos::common::v1alpha1::SegmentId;
use re_redap_client::{ApiResult, ConnectionClient, ConnectionRegistry};
use re_server::ServerHandle;
use re_uri::external::url::Host;
// pub use viewer_section::GetSection;
pub use viewer_section::ViewerSection;

pub struct TestServer {
    server_handle: Option<ServerHandle>,
    port: u16,
}

impl TestServer {
    pub async fn spawn() -> Self {
        // Get a random free port
        let port = get_free_port();

        let args = re_server::Args {
            host: "127.0.0.1".to_owned(),
            port,
            ..Default::default()
        };
        let (server_handle, _) = args
            .create_server_handle()
            .await
            .expect("Can't create server");

        Self {
            server_handle: Some(server_handle),
            port,
        }
    }

    pub async fn with_test_data(self) -> (Self, SegmentId) {
        let url = self.add_test_data().await;
        (self, url)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn client(&self) -> ApiResult<ConnectionClient> {
        let origin = re_uri::Origin {
            host: Host::Domain("localhost".to_owned()),
            port: self.port,
            scheme: re_uri::Scheme::RerunHttp,
        };
        ConnectionRegistry::new_without_stored_credentials()
            .client(origin)
            .await
    }

    pub async fn add_test_data(&self) -> SegmentId {
        let client = self.client().await.expect("Failed to connect");
        test_data::load_test_data(client)
            .await
            .expect("Failed to load test data")
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let server_handle = self
            .server_handle
            .take()
            .expect("Server handle not initialized");
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                server_handle.shutdown_and_wait().await;
            });
        });
    }
}

/// Get a free port from the OS.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    addr.port()
}
