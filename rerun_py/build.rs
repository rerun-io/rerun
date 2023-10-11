fn main() {
    // Required for `cargo build` to work on mac: https://pyo3.rs/v0.14.2/building_and_distribution.html#macos
    pyo3_build_config::add_extension_module_link_args();

    re_build_tools::export_build_info_vars_for_crate("rerun_py");
}
