//! Decide whether or not to enable the `hot_reload_design_tokens` feature.

#![expect(clippy::unwrap_used)]

fn main() {
    use re_build_tools::Environment;

    let environment = Environment::detect();
    let is_release = cfg!(not(debug_assertions)); // This works
    let is_test = cfg!(feature = "testing");

    // DO NOT USE `cfg!` for this, that would give you the host's platform!
    let targets_wasm =
        re_build_tools::get_and_track_env_var("CARGO_CFG_TARGET_FAMILY").unwrap() == "wasm";

    println!("cargo::rustc-check-cfg=cfg(hot_reload_design_tokens)");

    let hot_reload_design_tokens = environment == Environment::DeveloperInWorkspace
        && !is_release
        && !targets_wasm
        && !is_test;
    if hot_reload_design_tokens {
        println!("cargo:rustc-cfg=hot_reload_design_tokens");
    }
}
