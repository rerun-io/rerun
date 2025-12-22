fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/main/building-and-distribution#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");

    // Prevent accidental slow builds via `uv pip install` or `uv sync`.
    // These use isolated build environments which are very slow.
    // Direct `maturin develop` or `maturin build` invocations are fast and allowed.
    //
    // We detect isolated builds by checking `PYO3_PYTHON` - in an isolated build,
    // it points to a temp directory like ~/.cache/uv/builds-v0/.tmp*/
    // or a pip build-env directory.
    if re_build_tools::is_tracked_env_var_set("RERUN_BUILDING_WHEEL")
        && is_isolated_build_environment()
    {
        eprintln!();
        eprintln!("ERROR: rerun-sdk should not be built via `uv pip install` or `uv sync`.");
        eprintln!("       This uses an isolated build environment which is very slow.");
        eprintln!();
        eprintln!("       Instead, use `pixi run py-build` or `maturin develop`:");
        eprintln!();
        eprintln!(
            "           RERUN_ALLOW_MISSING_BIN=1 maturin develop --uv --manifest-path rerun_py/Cargo.toml"
        );
        eprintln!();
        eprintln!(
            "       Then use `uv sync --inexact --no-install-package rerun-sdk` to install other dependencies."
        );
        std::process::exit(1);
    }

    // Fail if bin/rerun is missing and we haven't specified it's ok.
    if re_build_tools::is_tracked_env_var_set("RERUN_BUILDING_WHEEL")
        && !re_build_tools::is_tracked_env_var_set("RERUN_ALLOW_MISSING_BIN")
    {
        #[cfg(target_os = "windows")]
        #[expect(clippy::unwrap_used)]
        let rerun_bin = std::env::current_dir()
            .unwrap()
            .join("rerun_sdk/rerun_cli/rerun.exe");

        #[cfg(not(target_os = "windows"))]
        let rerun_bin = std::env::current_dir()
            .expect("std::env::current_dir() failed")
            .join("rerun_sdk/rerun_cli/rerun");

        if !rerun_bin.exists() {
            eprintln!("ERROR: Expected to find `rerun` at `{rerun_bin:?}`.");
            std::process::exit(1);
        }
    }
}

/// Detect if we're in an isolated PEP 517 build environment.
///
/// When pip or uv builds a package, they create an isolated virtual environment
/// in a temporary directory. We can detect this by checking the `PYO3_PYTHON` path
/// which maturin sets to the Python interpreter being used.
///
/// Known patterns for isolated build environments:
/// - uv: `~/.cache/uv/builds-v0/.tmp*/bin/python`
/// - pip: `*/build-env-*/bin/python` or similar temp patterns
fn is_isolated_build_environment() -> bool {
    let python_path =
        re_build_tools::get_and_track_env_var("PYO3_PYTHON").unwrap_or_else(|_| String::new());

    if python_path.is_empty() {
        return false;
    }

    // uv isolated builds use ~/.cache/uv/builds-v0/.tmp*/
    if python_path.contains(".cache/uv/builds") {
        return true;
    }

    // pip isolated builds use build-env directories
    if python_path.contains("build-env") {
        return true;
    }

    // Generic pattern: temp directories with .tmp prefix in cache paths
    if python_path.contains("/.tmp") && python_path.contains("cache") {
        return true;
    }

    false
}
