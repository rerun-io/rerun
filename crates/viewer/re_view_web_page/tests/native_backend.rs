#![cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]

use re_view_web_page::native_backend::{NativeWebViewBackend, NativeWebViewError};

#[test]
fn native_backend_reports_missing_parent_window_without_panicking() {
    let result =
        NativeWebViewBackend::default().create_without_parent_for_smoke_test("https://example.com");

    assert!(matches!(
        result,
        Err(NativeWebViewError::MissingParentWindow)
    ));
}
