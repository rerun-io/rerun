use insta::with_settings;
use re_integration_test::{TestServer, load_test_data};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
pub fn integration_test() {
    let _server = TestServer::spawn();
    let test_output = load_test_data();

    insta::assert_snapshot!(test_output);
}
