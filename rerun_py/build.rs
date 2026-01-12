fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/main/building-and-distribution#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");

    // Prevent builds without PYO3_CONFIG_FILE in isolated environments.
    // When uv or pip builds a package, they create an isolated virtual environment
    // with a temporary Python path. Without PYO3_CONFIG_FILE, this causes cargo
    // cache invalidation on every build.
    //
    // The repository is configured to automatically generate pyo3-build.cfg via
    // pixi activation scripts. If you see this error, either:
    // 1. Run any `pixi run` command first (generates the config automatically)
    // 2. Run `pixi run ensure-pyo3-build-cfg ` to generate it manually
    if re_build_tools::is_tracked_env_var_set("RERUN_BUILDING_WHEEL")
        && is_isolated_build_environment()
    {
        eprintln!();
        eprintln!("ERROR: Missing PYO3_CONFIG_FILE for isolated build environment.");
        eprintln!();
        eprintln!("       The pyo3-build.cfg file is required for stable cargo caching.");
        eprintln!("       This file is normally generated automatically by pixi activation.");
        eprintln!();
        eprintln!("       To fix this, run any pixi command first:");
        eprintln!();
        eprintln!("           pixi run py-build");
        eprintln!();
        eprintln!("       Or generate the config manually:");
        eprintln!();
        eprintln!("           pixi run ensure-pyo3-build-cfg ");
        eprintln!();
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

/// Detect if we're in a problematic isolated PEP 517 build environment.
///
/// When pip or uv builds a package, they create an isolated virtual environment
/// in a temporary directory. We can detect this by checking the `PYO3_PYTHON` path
/// which maturin sets to the Python interpreter being used.
///
/// However, if `PYO3_CONFIG_FILE` is set, pyo3 uses that config instead of querying
/// `PYO3_PYTHON`, so the isolated environment is no longer problematic for caching.
///
/// Known patterns for isolated build environments:
/// - uv: `~/.cache/uv/builds-v0/.tmp*/bin/python`
/// - pip: `*/build-env-*/bin/python` or similar temp patterns
fn is_isolated_build_environment() -> bool {
    // If PYO3_CONFIG_FILE is set, pyo3 uses stable config regardless of PYO3_PYTHON,
    // so isolated builds won't cause cache invalidation.
    if re_build_tools::get_and_track_env_var("PYO3_CONFIG_FILE").is_ok() {
        return false;
    }

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
