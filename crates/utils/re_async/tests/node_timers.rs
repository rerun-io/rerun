#![cfg(target_arch = "wasm32")]

use std::time::Duration;

use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_node_experimental);

// Runs in Node to verify that sleep works without a browser window.
#[wasm_bindgen_test]
async fn sleeps_without_a_window() {
    assert!(web_sys::window().is_none());
    re_async::sleep(Duration::ZERO).await;
}
