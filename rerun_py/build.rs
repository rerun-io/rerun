fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/main/building-and-distribution#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");

    // Fail if bin/rerun is missing and we haven't specified it's ok.
    if re_build_tools::get_and_track_env_var("RERUN_BUILDING_WHEEL").unwrap_or_default() == "1"
        && re_build_tools::get_and_track_env_var("RERUN_ALLOW_MISSING_BIN").unwrap_or_default()
            != "1"
    {
        #[cfg(target_os = "windows")]
        let rerun_bin = std::env::current_dir()
            .unwrap()
            .join("rerun_sdk/bin/rerun.exe");

        #[cfg(not(target_os = "windows"))]
        let rerun_bin = std::env::current_dir().unwrap().join("rerun_sdk/bin/rerun");

        if !rerun_bin.exists() {
            eprintln!("ERROR: Expected to find `rerun` at `{rerun_bin:?}`.");
            std::process::exit(1);
        }
    }
}
