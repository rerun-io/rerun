use std::process::Stdio;

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

    if std::env::var("PROFILE").unwrap() == "release" {
        std::process::Command::new("../../scripts/build_web.sh")
            .arg("--optimize")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to build viewer for web");
    } else {
        std::process::Command::new("../../scripts/build_web.sh")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to build viewer for web");
    }
}
