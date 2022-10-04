use std::{ffi::OsString, process::Stdio};

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

    if std::env::var("CARGO_FEATURE___CI").is_ok() {
        // This saves a lot of CI time.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
    } else {
        eprintln!("Build web viewer wasm…");

        let mut cmd = std::process::Command::new("../../scripts/build_web.sh");

        if std::env::var("PROFILE").unwrap() == "release" {
            cmd.arg("--optimize");
        }

        // Get rid of everything cargo-related: we really don't want the cargo invocation
        // from build_web.sh to catch on some configuration variables that are really not
        // its concern!
        let env = cmd
            .get_envs()
            .filter(|(k, _)| !k.to_string_lossy().starts_with("CARGO"))
            .map(|(k, v)| (k.to_owned(), v.map_or_else(OsString::new, |v| v.to_owned())))
            .collect::<Vec<_>>();

        let output = cmd
            .envs(env)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to build viewer for web");

        eprintln!("status: {}", output.status);

        assert!(output.status.success());
    }
}
