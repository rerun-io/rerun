fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/main/building-and-distribution#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");

    // Prevent accidental slow builds via `uv pip install`.
    // rerun-sdk should only be built via `maturin develop` which is much faster.
    // When building via maturin develop, set RERUN_MATURIN_BUILD=1.
    if re_build_tools::is_tracked_env_var_set("RERUN_BUILDING_WHEEL")
        && !re_build_tools::is_tracked_env_var_set("RERUN_MATURIN_BUILD")
    {
        eprintln!();
        eprintln!("ERROR: rerun-sdk should not be built via `uv pip install` or `uv sync`.");
        eprintln!("       This uses an isolated build environment which is very slow.");
        eprintln!();
        eprintln!("       Instead, use `pixi run py-build` or `maturin develop`:");
        eprintln!();
        eprintln!(
            "           RERUN_MATURIN_BUILD=1 RERUN_ALLOW_MISSING_BIN=1 uv run maturin develop --uv --manifest-path rerun_py/Cargo.toml"
        );
        eprintln!();
        eprintln!(
            "       Then use `uv sync --inexact --no-install-workspace` to install other dependencies."
        );
        eprintln!();
        eprintln!("       If you really need to build via uv, set RERUN_MATURIN_BUILD=1.");
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
