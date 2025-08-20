use std::process::{Child, Command, Stdio};

pub struct TestServer {
    server: Child,
}

impl TestServer {
    pub fn spawn() -> Self {
        let mut build = Command::new("pixi");
        build.args(["run", "rerun", "--help"]);
        build.stdout(Stdio::null());
        build.spawn().unwrap().wait_for_success();

        let mut server = Command::new("../../../target_pixi/debug/rerun");
        server.args(["server"]);
        let mut server = server.spawn().unwrap();

        Self { server }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        sigint(&mut self.server)
    }
}

/// Send SIGINT and wait for the child process to exit successfully.
pub fn sigint(child: &mut Child) {
    if let Err(e) = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGINT,
    ) {
        eprintln!("Failed to send SIGINT to process {}: {e}", child.id());
    }

    child.wait_for_success();
}

/// Run re_integration.py to load some test data.
pub fn load_test_data() -> String {
    let mut script = Command::new("pixi");
    script.args(["run", "-e", "py", "python", "tests/re_integration.py"]);
    let mut output = script.output().unwrap().stdout;
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
