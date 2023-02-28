use std::{path::Path, process::Stdio};

// Port of build_web.sh
pub fn build(release: bool) {
    eprintln!("Building web viewer wasm…");

    let repository_root_dir = format!("{}/../..", std::env!("CARGO_MANIFEST_DIR"));

    let crate_name = "re_viewer";
    let build_dir = format!("{repository_root_dir}/web_viewer");

    assert!(
        Path::new(&build_dir).exists(),
        "Failed to find dir {build_dir}. CWD: {:?}, CARGO_MANIFEST_DIR: {:?}",
        std::env::current_dir(),
        std::env!("CARGO_MANIFEST_DIR")
    );

    // Clean previous version of what we are building:
    let wasm_path = Path::new(&build_dir).join([crate_name, "_bg.wasm"].concat());
    std::fs::remove_file(wasm_path.clone()).ok();
    let js_path = Path::new(&build_dir).join([crate_name, ".js"].concat());
    std::fs::remove_file(js_path).ok();

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let target_wasm = format!("{}_wasm", metadata.target_directory);

    let root_dir = metadata.target_directory.parent().unwrap();

    // --------------------------------------------------------------------------------
    // Compile rust to wasm

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(root_dir);
    cmd.args([
        "build",
        "--target-dir",
        &target_wasm,
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

    let target_path = Path::new(&target_wasm)
        .join("wasm32-unknown-unknown")
        .join(build)
        .join(target_name);

    let mut cmd = std::process::Command::new("wasm-bindgen");
    cmd.current_dir(root_dir);
    cmd.args([
        target_path.to_str().unwrap(),
        "--out-dir",
        &build_dir,
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
    eprintln!(
        "wasm-bindgen stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success());

    // --------------------------------------------------------------------------------
    // Optimize the wasm

    if release {
        let wasm_path = wasm_path.to_str().unwrap();

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
        eprintln!(
            "wasm-opt stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(output.status.success());
    }

    eprintln!("Finished {wasm_path:?}");
}
