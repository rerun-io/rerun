//! Integration tests for rerun and the in memory server.

use std::net::{SocketAddr, TcpListener};
use std::process::Command;

use re_server::{ServerBuilder, ServerHandle};
use tokio::task::spawn_blocking;
use ureq::OrAnyStatus as _;

pub struct TestServer {
    server_handle: ServerHandle,
    port: u16,
}

impl TestServer {
    pub async fn spawn() -> Self {
        // Get a random free port
        let port = get_free_port();

        println!("Spawning server on port {port}");

        let server_builder =
            ServerBuilder::default().with_address(SocketAddr::from(([0, 0, 0, 0], port)));
        let server = server_builder.build();
        let mut server_handle = server.start();

        server_handle
            .wait_for_ready()
            .await
            .expect("Can't start server");

        println!("Server ready on port {port}");

        // // First build the binary:
        // let mut build = Command::new("pixi");
        // build.args(["run", "rerun-build"]);
        // build.stdout(Stdio::null());
        // build
        //     .spawn()
        //     .expect("Failed to start pixi")
        //     .wait_for_success();

        // let mut server = Command::new("../../../target_pixi/debug/rerun");
        // server.args(["server", "--port", &port.to_string()]);
        // let server = server.spawn().expect("Failed to start rerun server");

        let mut success = false;
        let probe_url = format!("http://127.0.0.1:{port}");
        for _ in 0..50 {
            println!("Probing {probe_url}");
            let result = spawn_blocking(move || {
                ureq::get(&format!("http://127.0.0.1:{port}"))
                    .call()
                    .or_any_status()
            })
            .await;
            println!("Result: {result:?}");
            if result.is_ok() {
                success = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(success, "Failed to connect to rerun server");

        println!("Server answers on port {port}");

        Self {
            server_handle,
            port,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

// impl Drop for TestServer {
//     fn drop(&mut self) {
//         self.server_handle.
//         sigint_and_wait(&mut self.server);
//     }
// }

/// Send SIGINT and wait for the child process to exit successfully.
// pub fn sigint_and_wait(child: &mut Child) {
//     if let Err(err) = nix::sys::signal::kill(
//         nix::unistd::Pid::from_raw(child.id() as i32),
//         nix::sys::signal::Signal::SIGINT,
//     ) {
//         eprintln!("Failed to send SIGINT to process {}: {err}", child.id());
//     }

//     child.wait_for_success();
// }

/// Get a free port from the OS.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    addr.port()
}

/// Run `re_integration.py` to load some test data.
pub async fn load_test_data(port: u16) -> String {
    spawn_blocking(move || {
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
            .expect("Failed to run re_integration.py script");
        let stderr = String::from_utf8(output.stderr).expect("Failed to convert stderr to string");
        let stdout = String::from_utf8(output.stdout).expect("Failed to convert output to string");

        format!("{}\n\n{}", stdout.trim(), stderr.trim())
    })
    .await
    .expect("Failed to run re_integration.py script")
}

// trait ChildExt {
//     /// ## Panics
//     /// If the child process does not exit successfully.
//     fn wait_for_success(&mut self);
// }

// impl ChildExt for std::process::Child {
//     fn wait_for_success(&mut self) {
//         let status = self.wait().expect("Failed to wait on child process");
//         assert!(
//             status.success(),
//             "Child process did not exit successfully: {status:?}"
//         );
//     }
// }
