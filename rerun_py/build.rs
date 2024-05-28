fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/main/building-and-distribution#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");

    // Fail if bin/rerun is missing and we haven't specified it's ok.
    if re_build_tools::is_tracked_env_var_set("RERUN_BUILDING_WHEEL")
        && !re_build_tools::is_tracked_env_var_set("RERUN_ALLOW_MISSING_BIN")
    {
        #[cfg(target_os = "windows")]
        #[allow(clippy::unwrap_used)]
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
