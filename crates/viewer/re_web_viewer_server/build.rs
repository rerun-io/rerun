fn main() {
    // https://blog.rust-lang.org/2024/05/06/check-cfg.html
    // See `Cargo.toml` docs for info about `__disable_server` and `RERUN_DISABLE_WEB_VIEWER_SERVER`.
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");

    let disable_web_viewer_server =
        re_build_tools::is_tracked_env_var_set("RERUN_DISABLE_WEB_VIEWER_SERVER")
            || cfg!(feature = "__disable_server");

    if disable_web_viewer_server {
        println!("cargo::rustc-cfg=disable_web_viewer_server");
    }

    let needs_wasm = !disable_web_viewer_server;
    if needs_wasm {
        let viewer_js_path = std::path::Path::new("./web_viewer/re_viewer.js");
        let viewer_wasm_path = std::path::Path::new("./web_viewer/re_viewer_bg.wasm");

        assert!(
            viewer_js_path.exists() && viewer_wasm_path.exists(),
            "Web viewer not found, run `pixi run rerun-build-web` to build it!"
        );
    }
}
