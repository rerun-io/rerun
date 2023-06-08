fn main() {
    re_build_tools::rebuild_if_crate_changed("re_analytics");
    re_build_tools::export_env_vars();
}
