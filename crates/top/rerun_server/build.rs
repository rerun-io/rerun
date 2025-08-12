fn main() {
    // So one can do `CI=1 pixi run redap-server` to see the final result.
    _ = re_build_tools::get_and_track_env_var("CI");
    _ = re_build_tools::get_and_track_env_var("IS_IN_RERUN_WORKSPACE");

    re_build_tools::export_build_info_vars_for_crate("rerun_server");
}
