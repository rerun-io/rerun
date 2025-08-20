#[test]
pub fn integration_test() {
    let mut server = std::process::Command::new("pixi");
    server.args(["run", "rerun", "server"]);
    let mut server = server.spawn().unwrap();

    let mut script = std::process::Command::new(
        "pixi run python tests/rust/re_integration_test/tests/re_integration.py",
    );
    let mut script = script.spawn().unwrap();

    script.wait().unwrap();

    server.wait().unwrap();
}
