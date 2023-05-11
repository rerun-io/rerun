fn main() {
    if std::env::var("IS_IN_RERUN_WORKSPACE") == Ok("yes".to_owned()) {
        // During local development it is useful if the version string gets updated
        // whenever the binary is re-linked (e.g. when a dependency changes).
        // This is a glorious hack to achieve that:
        println!("cargo:rerun-if-changed=this/path/does/not/exist");
        // See https://github.com/rerun-io/rerun/issues/2086 for more
    }

    re_build_build_info::export_env_vars();
}
