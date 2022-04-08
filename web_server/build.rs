// Mapping to cargo:rerun-if-changed with glob support
fn rerun_if_changed(path: &str) {
    for path in glob::glob(path).unwrap() {
        println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
    }
}

fn main() {
    // Rebuild the web-viewer WASM,
    // because the web_server library bundles it with `include_bytes!`

    rerun_if_changed("../docs/favicon.ico");
    rerun_if_changed("../docs/index.html");
    rerun_if_changed("../docs/sw.js");
    rerun_if_changed("../viewer/Cargo.toml");
    rerun_if_changed("../viewer/src/**/*.rs");

    // Disabled, because it hangs :(
    // std::process::Command::new("../viewer/build_web.sh")
    //     .output()
    //     .expect("failed to build viewer for web");
}
