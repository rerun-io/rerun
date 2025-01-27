use std::path::Path;

fn main() {
    // https://blog.rust-lang.org/2024/05/06/check-cfg.html
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");

    let viewer_js_path = Path::new("./web_viewer/re_viewer.js");
    let viewer_wasm_path = Path::new("./web_viewer/re_viewer_bg.wasm");

    assert!(
        viewer_js_path.exists() && viewer_wasm_path.exists(),
        "Web viewer not found, run `pixi run rerun-build-web` to build it!"
    );
}
