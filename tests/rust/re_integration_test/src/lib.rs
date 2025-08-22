//! Integration tests for rerun and the in memory server.

mod test_data;

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};

use re_grpc_client::{ConnectionClient, ConnectionError, ConnectionRegistry};
use re_uri::external::url::Host;
use ureq::OrAnyStatus as _;

pub struct TestServer {
    server: Child,
    port: u16,
}

impl TestServer {
    pub fn spawn() -> Self {
        // Get a random free port
        let port = get_free_port();

        // First build the binary:
        let mut build = Command::new("pixi");
        build.args(["run", "rerun-build"]);
        build.stdout(Stdio::null());
        build
            .spawn()
            .expect("Failed to start pixi")
            .wait_for_success();

        let mut server = Command::new("../../../target_pixi/debug/rerun");
        server.args(["server", "--port", &port.to_string()]);
        let server = server.spawn().expect("Failed to start rerun server");

        let mut success = false;
        for _ in 0..50 {
            let result = ureq::get(&format!("http://localhost:{port}"))
                .call()
                .or_any_status();
            if result.is_ok() {
                success = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(success, "Failed to connect to rerun server");

        Self { server, port }
    }

    pub async fn with_test_data(mut self) -> Self {
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

impl Drop for TestServer {
    fn drop(&mut self) {
        kill_and_wait(&mut self.server);
    }
}

/// Send SIGINT and wait for the child process to exit successfully.
pub fn kill_and_wait(child: &mut Child) {
    if let Err(err) = child.kill() {
        eprintln!("Failed to kill process {}: {err}", child.id());
    }
    child.wait().expect("Failed to wait on child process");
}

/// Get a free port from the OS.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    addr.port()
}

trait ChildExt {
    /// ## Panics
    /// If the child process does not exit successfully.
    fn wait_for_success(&mut self);
}

impl ChildExt for std::process::Child {
    fn wait_for_success(&mut self) {
        let status = self.wait().expect("Failed to wait on child process");
        assert!(
            status.success(),
            "Child process did not exit successfully: {status:?}"
        );
    }
}
