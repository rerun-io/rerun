//! Integration tests for rerun and the in memory server.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};

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

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        sigint_and_wait(&mut self.server);
    }
}

/// Send SIGINT and wait for the child process to exit successfully.
pub fn sigint_and_wait(child: &mut Child) {
    if let Err(err) = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGINT,
    ) {
        eprintln!("Failed to send SIGINT to process {}: {err}", child.id());
    }

    child.wait_for_success();
}

/// Get a free port from the OS.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    addr.port()
}

/// Run `re_integration.py` to load some test data.
pub fn load_test_data(port: u16) -> String {
    let url = format!("rerun+http://localhost:{port}");
    let mut script = Command::new("pixi");
    script.args([
        "run",
        "-e",
        "py",
        "python",
        "tests/re_integration.py",
        "--url",
        &url,
    ]);
    let output = script
        .output()
        .expect("Failed to run re_integration.py script")
        .stdout;
    String::from_utf8(output).expect("Failed to convert output to string")
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
