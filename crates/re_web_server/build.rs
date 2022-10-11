use cargo_metadata::{CargoOpt, MetadataCommand, Package, PackageId};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    process::Stdio,
};

// Mapping to cargo:rerun-if-changed with glob support
fn rerun_if_changed(path: &str) {
    for path in glob::glob(path).unwrap() {
        println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
    }
}

/// Generates all the appropriate `cargo:rerun-if-changed` lines so that all locally
/// patched dependency of package `pkg_name` are properly tracked.
fn rerun_if_patched(pkg_name: &str) {
    let metadata = MetadataCommand::new()
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let pkgs = metadata
        .packages
        .iter()
        .map(|pkg| (pkg.name.as_str(), pkg))
        .collect::<HashMap<_, _>>();
    let pkgs_workspace = metadata.workspace_packages();
    let pkgs_workspace = pkgs_workspace
        .iter()
        .map(|pkg| (pkg.name.as_str(), *pkg))
        .collect::<HashMap<_, _>>();

    let mut tracked = HashSet::new();

    let pkg = pkgs.values().find(|pkg| pkg.name == pkg_name).unwrap();
    track_patched_deps(&pkgs, &pkgs_workspace, &mut tracked, pkg);
}

/// Recursively walk the tree of dependencies of the given `root` package, making sure
/// to track all potentially modified, locally patched dependencies.
fn track_patched_deps(
    pkgs: &HashMap<&str, &Package>,
    pkgs_workspace: &HashMap<&str, &Package>,
    tracked: &mut HashSet<PackageId>,
    root: &Package,
) {
    for dep_pkg in root
        .dependencies
        .iter()
        // NOTE: We'd like to just use `dep.source`/`dep.path`, unfortunately they do not
        // account for crate patches at this level, so we build our own little index
        // and use that instead.
        .filter_map(|dep| pkgs.get(dep.name.as_str()))
    {
        let is_in_workspace = pkgs_workspace.contains_key(dep_pkg.name.as_str());
        let exists_on_local_disk = dep_pkg.source.is_none();
        if !is_in_workspace && exists_on_local_disk {
            let mut dep_path = dep_pkg.manifest_path.clone();
            dep_path.pop();

            rerun_if_changed(dep_path.join("Cargo.toml").as_ref());
            rerun_if_changed(dep_path.join("Cargo.lock").as_ref());
            rerun_if_changed(dep_path.join("**/*.rs").as_ref());
        }

        if tracked.insert(dep_pkg.id.clone()) {
            track_patched_deps(pkgs, pkgs_workspace, tracked, dep_pkg);
        }
    }
}

fn main() {
    // Rebuild the web-viewer WASM,
    // because the web_server library bundles it with `include_bytes!`

    rerun_if_changed("../../web_viewer/favicon.ico");
    rerun_if_changed("../../web_viewer/index.html");
    rerun_if_changed("../../web_viewer/sw.js");

    rerun_if_patched("re_viewer");

    if std::env::var("CARGO_FEATURE___CI").is_ok() {
        // This saves a lot of CI time.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
    } else {
        eprintln!("Build web viewer wasm…");

        let mut cmd = std::process::Command::new("../../scripts/build_web.sh");

        if std::env::var("PROFILE").unwrap() == "release" {
            cmd.arg("--optimize");
        }

        // Get rid of everything cargo-related: we really don't want the cargo invocation
        // from build_web.sh to catch on some configuration variables that are really not
        // its concern!
        let env = cmd
            .get_envs()
            .filter(|(k, _)| !k.to_string_lossy().starts_with("CARGO"))
            .map(|(k, v)| (k.to_owned(), v.map_or_else(OsString::new, |v| v.to_owned())))
            .collect::<Vec<_>>();

        let output = cmd
            .envs(env)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to build viewer for web");

        eprintln!("status: {}", output.status);

        assert!(output.status.success());
    }
}
