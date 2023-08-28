fn main() {
    re_build_tools::rebuild_if_crate_changed("re_data_source");
    re_build_tools::export_env_vars();
}
