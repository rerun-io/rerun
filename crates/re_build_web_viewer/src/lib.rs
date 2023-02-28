use std::process::Stdio;

use cargo_metadata::camino::Utf8PathBuf;

fn target_directory() -> Utf8PathBuf {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .unwrap();
    metadata.target_directory
}

// Port of build_web.sh
pub fn build(release: bool) {
    eprintln!("Building web viewer wasm…");

    let crate_name = "re_viewer";

    // Where cargo is building to
    let target_wasm_dir = Utf8PathBuf::from(format!("{}_wasm", target_directory()));

    // Repository root
    let root_dir = target_wasm_dir.parent().unwrap();

    // Where we will place the final .wasm and .js artifacts.
    let build_dir = root_dir.join("web_viewer");

    assert!(
        build_dir.exists(),
        "Failed to find dir {build_dir}. CWD: {:?}, CARGO_MANIFEST_DIR: {:?}",
        std::env::current_dir(),
        std::env!("CARGO_MANIFEST_DIR")
    );

    // Clean previous version of what we are building:
    let wasm_path = build_dir.join([crate_name, "_bg.wasm"].concat());
    std::fs::remove_file(wasm_path.clone()).ok();
    let js_path = build_dir.join([crate_name, ".js"].concat());
    std::fs::remove_file(js_path).ok();

    // --------------------------------------------------------------------------------
    // Compile rust to wasm

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(root_dir);
    cmd.args([
        "build",
        "--target-dir",
        target_wasm_dir.as_str(),
        "-p",
        crate_name,
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]);
    cmd.env("RUSTFLAGS", "--cfg=web_sys_unstable_apis");
    cmd.env("CARGO_ENCODED_RUSTFLAGS", "");

    if release {
        cmd.arg("--release");
    }

    eprintln!("wasm build cmd: {cmd:?}");

    let output = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect("failed to compile re_viewer for wasm32");

    eprintln!("compile status: {}", output.status);
    eprintln!(
        "compile stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success());

    // --------------------------------------------------------------------------------
    // Generate JS bindings

    let build = if release { "release" } else { "debug" };

    let target_name = [crate_name, ".wasm"].concat();

    let target_path = target_wasm_dir
        .join("wasm32-unknown-unknown")
        .join(build)
        .join(target_name);

    let mut cmd = std::process::Command::new("wasm-bindgen");
    cmd.current_dir(root_dir);
    cmd.args([
        target_path.as_str(),
        "--out-dir",
        build_dir.as_str(),
        "--no-modules",
        "--no-typescript",
    ]);

    eprintln!("wasm-bindgen cmd: {cmd:?}");

    let output = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .unwrap_or_else(|err| panic!("Failed to generate JS bindings: {err}. target_path: {target_path:?}, build_dir: {build_dir}"));

    eprintln!("wasm-bindgen status: {}", output.status);
    if !output.stderr.is_empty() {
        eprintln!(
            "wasm-bindgen stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert!(output.status.success());

    // --------------------------------------------------------------------------------
    // Optimize the wasm

    if release {
        let wasm_path = wasm_path.as_str();

        // to get wasm-opt:  apt/brew/dnf install binaryen
        let mut cmd = std::process::Command::new("wasm-opt");
        cmd.current_dir(root_dir);
        cmd.args([wasm_path, "-O2", "--fast-math", "-o", wasm_path]);

        eprintln!("wasm-opt cmd: {cmd:?}");

        let output = cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to optimize wasm");

        eprintln!("wasm-opt status: {}", output.status);
        if !output.stderr.is_empty() {
            eprintln!(
                "wasm-opt stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        assert!(output.status.success());
    }

    eprintln!("Finished {wasm_path:?}");
}
