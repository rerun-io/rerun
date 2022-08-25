// Mapping to cargo:rerun-if-changed with glob support
fn rerun_if_changed(path: &str) {
    for path in glob::glob(path).unwrap() {
        println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
    }
}

fn main() {
    // Rebuild the web-viewer WASM,
    // because the web_server library bundles it with `include_bytes!`

    rerun_if_changed("../../web_viewer/favicon.ico");
    rerun_if_changed("../../web_viewer/index.html");
    rerun_if_changed("../../web_viewer/sw.js");
    rerun_if_changed("../../crates/re_viewer/Cargo.toml");
    rerun_if_changed("../../crates/re_viewer/src/**/*.rs");

    // Disabled, because it hangs :(
    // std::process::Command::new("../../crates/re_viewer/build_web.sh")
    //     .output()
    //     .expect("failed to build viewer for web");
}
