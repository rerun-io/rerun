fn main() {
    re_build_build_info::rebuild_if_crate_changed("rerun");
    re_build_build_info::export_env_vars();
}
