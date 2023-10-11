#![allow(clippy::unwrap_used)]

use re_build_tools::{
    get_and_track_env_var, is_tracked_env_var_set, rebuild_if_crate_changed, rerun_if_changed,
};

fn should_run() -> bool {
    #![allow(clippy::match_same_arms)]
    use re_build_tools::Environment;

    match Environment::detect() {
        // We should build the web viewer before starting crate publishing
        Environment::PublishingCrates => false,

        // TODO(emilk): only build the web viewer explicitly on CI
        Environment::CI => true,

        Environment::DeveloperInWorkspace => true,

        // Definitely not
        Environment::UsedAsDependency => false,
    }
}

fn main() {
    if !should_run() {
        return;
    }

    // Rebuild the web-viewer Wasm,
    // because the web_server library bundles it with `include_bytes!`.

    rerun_if_changed("../../web_viewer/favicon.svg");
    rerun_if_changed("../../web_viewer/index.html");
    rerun_if_changed("../../web_viewer/sw.js");

    // We implicitly depend on re_viewer, which means we also implicitly depend on
    // all of its direct and indirect dependencies (which are potentially in-workspace
    // or patched!).
    rebuild_if_crate_changed("re_viewer");

    if get_and_track_env_var("CARGO_FEATURE___CI").is_ok() {
        // If the `__ci` feature is set we skip building the web viewer wasm, saving a lot of time.
        // This feature is set on CI (hence the name), but also with `--all-features`, which is set by rust analyzer, bacon, etc.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
    } else {
        let release = re_build_tools::get_and_track_env_var("PROFILE").unwrap() == "release";
        if let Err(err) =
            re_build_web_viewer::build(release, is_tracked_env_var_set("RERUN_BUILD_WEBGPU"))
        {
            panic!("Failed to build web viewer: {}", re_error::format(err));
        }
    }
}
