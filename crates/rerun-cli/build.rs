fn main() {
    re_build_tools::rebuild_if_crate_changed("rerun-cli");
    re_build_tools::export_env_vars();
}
