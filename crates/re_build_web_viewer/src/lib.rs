#![allow(clippy::unwrap_used)]

//! Build the Rerun web-viewer .wasm and generate the .js bindings for it.

use anyhow::Context as _;
use cargo_metadata::camino::Utf8PathBuf;

fn target_directory() -> Utf8PathBuf {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .features(cargo_metadata::CargoOpt::NoDefaultFeatures)
        .exec()
        .unwrap();
    metadata.target_directory
}

/// Build `re_viewer` as Wasm, generate .js bindings for it, and place it all into the `./web_viewer` folder.
pub fn build(release: bool, webgpu: bool) -> anyhow::Result<()> {
    eprintln!("Building web viewer wasm…");
    eprintln!("We assume you've already run ./scripts/setup_web.sh");

    let crate_name = "re_viewer";

    // Where we tell cargo to build to.
    // We want this to be different from the default target folder
    // in order to support recursive cargo builds (calling `cargo` from within a `build.rs`).
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

    let target_name = if release {
        crate_name.to_owned()
    } else {
        format!("{crate_name}_debug")
    };

    // The two files we are building:
    let wasm_path = build_dir.join(format!("{target_name}_bg.wasm"));
    let js_path = build_dir.join(format!("{target_name}.js"));

    // Clean old versions:
    std::fs::remove_file(wasm_path.clone()).ok();
    std::fs::remove_file(js_path).ok();

    // --------------------------------------------------------------------------------
    eprintln!("Compiling rust to wasm in {target_wasm_dir}…");

    let mut cmd = std::process::Command::new("cargo");
    cmd.args([
        "build",
        "--quiet",
        "--package",
        crate_name,
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--target-dir",
        target_wasm_dir.as_str(),
        "--no-default-features",
    ]);
    if webgpu {
        cmd.arg("--features=analytics");
    } else {
        cmd.arg("--features=analytics,webgl");
    }
    if release {
        cmd.arg("--release");
    }

    // This is required to enable the web_sys clipboard API which egui_web uses
    // https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Clipboard.html
    // https://rustwasm.github.io/docs/wasm-bindgen/web-sys/unstable-apis.html
    // Furthermore, it's necessary for unstable WebGPU apis to work.
    cmd.env("RUSTFLAGS", "--cfg=web_sys_unstable_apis");

    // When executing this script from a Rust build script, do _not_, under any circumstances,
    // allow pre-encoded `RUSTFLAGS` to leak into the current environment.
    // These pre-encoded flags are generally generated by Cargo itself when loading its
    // configuration from e.g. `$CARGO_HOME/config.toml`; which means they will contain
    // values that only make sense for the native target host, not for a wasm build.
    cmd.env("CARGO_ENCODED_RUSTFLAGS", "--cfg=web_sys_unstable_apis");

    eprintln!("> {cmd:?}");
    let status = cmd
        .current_dir(root_dir)
        .status()
        .context("Failed to build Wasm")?;
    assert!(status.success(), "Failed to build Wasm");

    // --------------------------------------------------------------------------------
    eprintln!("Generating JS bindings for wasm…");

    let build = if release { "release" } else { "debug" };

    let target_wasm_path = target_wasm_dir
        .join("wasm32-unknown-unknown")
        .join(build)
        .join(format!("{crate_name}.wasm"));

    // wasm-bindgen --target web target_wasm_path --no-typescript --out-name target_name --out-dir build_dir
    if let Err(err) = wasm_bindgen_cli_support::Bindgen::new()
        .no_modules(true)?
        .input_path(target_wasm_path.as_str())
        .typescript(false)
        .out_name(target_name.as_str())
        .generate(build_dir.as_str())
    {
        if err
            .to_string()
            .starts_with("cannot import from modules (`env`")
        {
            // Very common error: "cannot import from modules (`env`) with `--no-modules`"
            anyhow::bail!(
                "Failed to run wasm-bindgen: {err}. This is often because some dependency is calling `std::time::Instant::now()` or similar. You can try diagnosing this with:\n\
                wasm2wat {target_wasm_path} | rg '\"env\"'\n\
                wasm2wat {target_wasm_path} | rg 'call .now\\b' -B 20"
            );
        } else {
            return Err(err.context("Failed to run wasm-bindgen"));
        }
    }

    // --------------------------------------------------------------------------------

    if release {
        eprintln!("Optimizing wasm with wasm-opt…");

        // to get wasm-opt:  apt/brew/dnf install binaryen
        let mut cmd = std::process::Command::new("wasm-opt");

        // TODO(emilk): add `-g` to keep debug symbols; useful for profiling release builds in the in-browser profiler.
        cmd.args([wasm_path.as_str(), "-O2", "--output", wasm_path.as_str()]);

        eprintln!("> {cmd:?}");
        let status = cmd
            .current_dir(root_dir)
            .status()
            .context("Failed to run wasm-opt")?;
        assert!(status.success(), "Failed to run wasm-opt");
    }

    // --------------------------------------------------------------------------------

    eprintln!("Finished {wasm_path}");

    Ok(())
}
