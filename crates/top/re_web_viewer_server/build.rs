fn main() {
    // https://blog.rust-lang.org/2024/05/06/check-cfg.html
    // See `Cargo.toml` docs for info about `__disable_server` and `RERUN_DISABLE_WEB_VIEWER_SERVER`.
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");
    println!("cargo::rustc-check-cfg=cfg(trailing_web_viewer)");
    println!("cargo::rustc-check-cfg=cfg(external_web_viewer)");

    let disable_web_viewer_server =
        re_build_tools::is_tracked_env_var_set("RERUN_DISABLE_WEB_VIEWER_SERVER")
            || cfg!(feature = "__disable_server");

    if disable_web_viewer_server {
        println!("cargo::rustc-cfg=disable_web_viewer_server");
    }

    // When using trailing_web_viewer, we don't need the wasm at build time
    // because it will be appended to the binary in a post-processing step.
    let trailing_web_viewer = re_build_tools::is_tracked_env_var_set("RERUN_TRAILING_WEB_VIEWER")
        || cfg!(feature = "__trailing_web_viewer");

    if trailing_web_viewer {
        println!("cargo::rustc-cfg=trailing_web_viewer");
    }

    // When using external_web_viewer, we don't need the wasm at build time
    // because it will be loaded at runtime from a zip archive on disk
    // (e.g. one shipped inside a Python wheel).
    let external_web_viewer = re_build_tools::is_tracked_env_var_set("RERUN_EXTERNAL_WEB_VIEWER")
        || cfg!(feature = "__external_web_viewer");

    if external_web_viewer {
        println!("cargo::rustc-cfg=external_web_viewer");
    }

    assert!(
        disable_web_viewer_server || !(trailing_web_viewer && external_web_viewer),
        "RERUN_TRAILING_WEB_VIEWER and RERUN_EXTERNAL_WEB_VIEWER are mutually exclusive"
    );

    let needs_wasm = !disable_web_viewer_server && !trailing_web_viewer && !external_web_viewer;

    if needs_wasm {
        let viewer_js_path = std::path::Path::new("./web_viewer/re_viewer.js");
        let viewer_wasm_path = std::path::Path::new("./web_viewer/re_viewer_bg.wasm");

        assert!(
            viewer_js_path.exists() && viewer_wasm_path.exists(),
            "Web viewer not found, run `pixi run rerun-build-web` to build it!"
        );
    }
}
