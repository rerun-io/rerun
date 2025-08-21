use re_integration_test::{TestServer, load_test_data};

#[test]
pub fn integration_test() {
    let _server = TestServer::spawn();
    let test_output = load_test_data();

    insta::assert_snapshot!(test_output);
}
