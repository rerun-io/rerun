#![allow(clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package, PackageId};

/// Call from `build.rs` to trigger a rebuild whenever any source file of the given package
/// _or any of its dependencies_ changes, recursively.
///
/// This will work even if the package depends on crates that are outside of the workspace,
/// included with `path = …`
pub fn rebuild_if_crate_changed(pkg_name: &str) {
    if !is_tracked_env_var_set("IS_IN_RERUN_WORKSPACE") {
        // Only run if we are in the rerun workspace, not on users machines.
        return;
    }
    if is_tracked_env_var_set("RERUN_IS_PUBLISHING") {
        // We cannot run this during publishing.
        // We don't need to, and it can also can cause a Cargo.lock file to be generated.
        return;
    }

    let metadata = MetadataCommand::new()
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let mut files_to_watch = Default::default();

    let pkgs = Packages::from_metadata(&metadata);
    pkgs.track_implicit_dep(pkg_name, &mut files_to_watch);

    for path in &files_to_watch {
        rerun_if_changed(path);
    }
}

fn get_and_track_env_var(env_var_name: &str) -> Result<String, std::env::VarError> {
    println!("cargo:rerun-if-env-changed={env_var_name}");
    std::env::var(env_var_name)
}

fn is_tracked_env_var_set(env_var_name: &str) -> bool {
    let var = get_and_track_env_var(env_var_name).map(|v| v.to_lowercase());
    var == Ok("1".to_owned()) || var == Ok("yes".to_owned()) || var == Ok("true".to_owned())
}

fn rerun_if_changed(path: &std::path::Path) {
    // Make sure the file exists, otherwise we'll be rebuilding all the time.
    assert!(path.exists(), "Failed to find {path:?}");
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

fn rerun_if_changed_glob(path: &str, files_to_watch: &mut HashSet<PathBuf>) {
    // Workaround for windows verbatim paths not working with glob.
    // Issue: https://github.com/rust-lang/glob/issues/111
    // Fix: https://github.com/rust-lang/glob/pull/112
    // Fixed on upstream, but no release containing the fix as of writing.
    let path = path.trim_start_matches(r"\\?\");

    for path in glob::glob(path).unwrap() {
        files_to_watch.insert(path.unwrap());
    }
}

// ---

struct Packages<'a> {
    pkgs: HashMap<&'a str, &'a Package>,
}

impl<'a> Packages<'a> {
    pub fn from_metadata(metadata: &'a Metadata) -> Self {
        let pkgs = metadata
            .packages
            .iter()
            .map(|pkg| (pkg.name.as_str(), pkg))
            .collect::<HashMap<_, _>>();

        Self { pkgs }
    }

    /// Tracks an implicit dependency of the given name.
    ///
    /// This will generate all the appropriate `cargo:rerun-if-changed` clauses
    /// so that package `pkg_name` as well as all of it direct and indirect
    /// dependencies are properly tracked whether they are remote, in-workspace,
    /// or locally patched.
    pub fn track_implicit_dep(&self, pkg_name: &str, files_to_watch: &mut HashSet<PathBuf>) {
        let pkg = self.pkgs.get(pkg_name).unwrap_or_else(|| {
            let found_names: Vec<&str> = self.pkgs.values().map(|pkg| pkg.name.as_str()).collect();
            panic!("Failed to find package {pkg_name:?} among {found_names:?}")
        });

        // Track the root package itself
        {
            let mut path = pkg.manifest_path.clone();
            path.pop();

            // NOTE: Since we track the cargo manifest, past this point we only need to
            // account for locally patched dependencies.
            rerun_if_changed_glob(path.join("Cargo.toml").as_ref(), files_to_watch);
            rerun_if_changed_glob(path.join("**/*.rs").as_ref(), files_to_watch);
            rerun_if_changed_glob(path.join("**/*.wgsl").as_ref(), files_to_watch);
        }

        // Track all direct and indirect dependencies of that root package
        let mut tracked = HashSet::new();
        self.track_patched_deps(&mut tracked, pkg, files_to_watch);
    }

    /// Recursively walk the tree of dependencies of the given `root` package, making sure
    /// to track all potentially modified, locally patched dependencies.
    fn track_patched_deps(
        &self,
        tracked: &mut HashSet<PackageId>,
        root: &Package,
        files_to_watch: &mut HashSet<PathBuf>,
    ) {
        for dep_pkg in root
            .dependencies
            .iter()
            // NOTE: We'd like to just use `dep.source`/`dep.path`, unfortunately they do not
            // account for crate patches at this level, so we build our own little index
            // and use that instead.
            .filter_map(|dep| self.pkgs.get(dep.name.as_str()))
        {
            let exists_on_local_disk = dep_pkg.source.is_none();
            if exists_on_local_disk {
                let mut dep_path = dep_pkg.manifest_path.clone();
                dep_path.pop();

                rerun_if_changed_glob(dep_path.join("Cargo.toml").as_ref(), files_to_watch); // manifest too!
                rerun_if_changed_glob(dep_path.join("**/*.rs").as_ref(), files_to_watch);
            }

            if tracked.insert(dep_pkg.id.clone()) {
                self.track_patched_deps(tracked, dep_pkg, files_to_watch);
            }
        }
    }
}
