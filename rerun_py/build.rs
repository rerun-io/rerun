fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/v0.14.2/building_and_distribution.html#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::rebuild_if_crate_changed("rerun_py");
    re_build_tools::export_env_vars();
}
