//! Static files for the browser control panel in `web/`.

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use rerun::external::{re_error, re_log};

const INDEX_HTML: &str = include_str!("../../web/index.html");
const MAIN_JS: &str = include_str!("../../web/main.js");

/// The in-repo `@rerun-io/web-viewer` package, which must be built first (`pixi run js-build-base`).
///
/// It is read at request time, because the Wasm is too large to embed and changes independently of this example.
const WEB_VIEWER_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../rerun_js/web-viewer");

/// Routes for the page, its script, and the web viewer package.
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route(
            "/main.js",
            get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], MAIN_JS) }),
        )
        .route("/web-viewer/{file}", get(web_viewer_file))
}

async fn web_viewer_file(Path(file): Path<String>) -> Response {
    // Only these names are served, so a request cannot escape `WEB_VIEWER_DIR`.
    // `index.js` imports `./re_viewer` without an extension, hence the alias.
    let (file_name, content_type) = match file.as_str() {
        "index.js" => ("index.js", "text/javascript"),
        "re_viewer" | "re_viewer.js" => ("re_viewer.js", "text/javascript"),
        "re_viewer_bg.wasm" => ("re_viewer_bg.wasm", "application/wasm"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let path = std::path::Path::new(WEB_VIEWER_DIR).join(file_name);
    match tokio::fs::read(&path).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, content_type)], bytes).into_response(),
        Err(err) => {
            let msg = format!(
                "Failed to read web viewer file: {}. Build it with `pixi run js-build-base`.\nFile path: {}",
                re_error::format_ref(&err),
                path.display()
            );
            re_log::error!("{msg}");
            (StatusCode::NOT_FOUND, msg).into_response()
        }
    }
}
