#![allow(clippy::unwrap_used)]

use re_build_tools::{get_and_track_env_var, rebuild_if_crate_changed, rerun_if_changed};

fn should_run() -> bool {
    #![allow(clippy::match_same_arms)]
    use re_build_tools::Environment;

    if get_and_track_env_var("CARGO_FEATURE___CI").is_ok() {
        // If the `__ci` feature is set we skip building the web viewer wasm, saving a lot of time.
        // This feature is set on CI (hence the name), but also with `--all-features`, which is set by rust analyzer, bacon, etc.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
        return false;
    }

    match Environment::detect() {
        // We should build the web viewer before starting crate publishing
        Environment::PublishingCrates => false,

        // We build the web viewer in an explicit, separate step.
        Environment::RerunCI => false,

        // We build the web-viewer as an explicit step:
        // https://github.com/conda-forge/rerun-sdk-feedstock/blob/8a63484685d6697c638c0d45b78396f049d10ce7/recipe/build.sh#L18
        Environment::CondaBuild => false,

        // If a developer is iterating on the viewer, they don't want to manually recompile it.
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

    let release = if re_build_tools::get_and_track_env_var("PROFILE").unwrap() == "release" {
        re_build_web_viewer::Profile::Release
    } else {
        re_build_web_viewer::Profile::Debug
    };
    let target = re_build_web_viewer::Target::Browser;
    let build_dir = re_build_web_viewer::default_build_dir();
    if let Err(err) = re_build_web_viewer::build(release, target, &build_dir) {
        panic!("Failed to build web viewer: {}", re_error::format(err));
    }
}
