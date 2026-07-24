#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn redap_port() -> u16 {
    // TODO(grtlr): Move browser URL query parsing to `re_web`.
    let search = web_sys::window()
        .expect("test should run in a browser window")
        .location()
        .search()
        .expect("window location search should be available");

    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(key, value)| (key == "redap_port").then_some(value))
        .expect("redap_port query parameter should be set")
        .parse()
        .expect("redap_port should be a valid port")
}

#[wasm_bindgen_test]
async fn check_native_redap_server_version_endpoint() {
    re_log::setup_logging();

    let port = redap_port();
    let url = format!("http://127.0.0.1:{port}/version");

    let response = ehttp::fetch_async(ehttp::Request::get(url.clone()))
        .await
        .unwrap_or_else(|err| panic!("failed to get {url}: {err}"));
    let text = response.text();
    assert!(
        response.ok,
        "failed to get {url}: {} {} {text:?}",
        response.status, response.status_text
    );

    let text = text.expect("/version response should be valid UTF-8");
    assert!(
        text.starts_with("re_server "),
        "unexpected /version response: {text:?}"
    );
}
