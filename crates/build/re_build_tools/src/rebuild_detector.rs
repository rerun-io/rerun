#![expect(clippy::unwrap_used)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use cargo_metadata::camino::Utf8Path;
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package, PackageId};

use crate::should_output_cargo_build_instructions;

fn should_run() -> bool {
    #![expect(clippy::match_same_arms)]
    use super::Environment;

    match Environment::detect() {
        // We cannot run this during publishing,
        // we don't need to,
        // and it can also can cause a Cargo.lock file to be generated.
        Environment::PublishingCrates | Environment::CondaBuild => false,

        // Dependencies shouldn't change on CI, but who knows 🤷‍♂️
        Environment::RerunCI => true,

        // Yes - this is what we want tracking for.
        Environment::DeveloperInWorkspace => true,

        // Definitely not
        Environment::UsedAsDependency => false,
    }
}

/// Call from `build.rs` to trigger a rebuild whenever any source file of the given package
/// _or any of its dependencies_ changes, recursively.
///
/// This will work even if the package depends on crates that are outside of the workspace,
/// included with `path = …`
///
/// However, this is a complex beast, and may have bugs in it.
/// Maybe it is even causing spurious re-compiles (<https://github.com/rerun-io/rerun/issues/3266>).
pub fn rebuild_if_crate_changed(pkg_name: &str) {
    if !should_run() {
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

/// Read the environment variable and trigger a rebuild whenever the environment variable changes.
pub fn get_and_track_env_var(env_var_name: &str) -> Result<String, std::env::VarError> {
    if should_output_cargo_build_instructions() {
        println!("cargo:rerun-if-env-changed={env_var_name}");
    }
    std::env::var(env_var_name)
}

/// Read the environment variable and trigger a rebuild whenever the environment variable changes.
///
/// Returns `true` if that variable has been set to a truthy value.
pub fn is_tracked_env_var_set(env_var_name: &str) -> bool {
    match get_and_track_env_var(env_var_name) {
        Err(_) => false,
        Ok(value) => match value.to_lowercase().as_str() {
            "1" | "yes" | "true" => true,
            "0" | "no" | "false" => false,
            _ => {
                println!(
                    "cargo::error=Failed to understand boolean env-var {env_var_name}={value}"
                );
                false
            }
        },
    }
}

/// Call from `build.rs` to trigger a rebuild whenever the file at `path` changes.
///
/// This requires the file to exist, which may or may not be what you want!
pub fn rerun_if_changed(path: impl AsRef<Path>) {
    let path = path.as_ref();
    // Make sure the file exists, otherwise we'll be rebuilding all the time.
    assert!(path.exists(), "Failed to find {path:?}");
    if should_output_cargo_build_instructions() {
        println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    }
}

/// Call from `build.rs` to trigger a rebuild whenever the file at `path` changes, or it doesn't
/// exist.
pub fn rerun_if_changed_or_doesnt_exist(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if should_output_cargo_build_instructions() {
        println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    }
}

/// Call from `build.rs` to trigger a rebuild whenever any of the files identified by the given
/// globbed `path` change.
pub fn rerun_if_changed_glob(path: impl AsRef<Path>, files_to_watch: &mut HashSet<PathBuf>) {
    let path = path.as_ref();

    // Workaround for windows verbatim paths not working with glob.
    // Issue: https://github.com/rust-lang/glob/issues/111
    // Fix: https://github.com/rust-lang/glob/pull/112
    // Fixed on upstream, but no release containing the fix as of writing.
    let path = path.to_str().unwrap().trim_start_matches(r"\\?\");

    for path in glob::glob(path).unwrap() {
        files_to_watch.insert(path.unwrap());
    }
}

/// Writes `content` to a file iff it differs from what's already there.
///
/// This prevents recursive feedback loops where one generates source files from build.rs, which in
/// turn triggers `cargo`'s implicit `rerun-if-changed=src/**` clause.
//
// TODO(cmc): use the same source tracking system as re_sdk_types* instead
pub fn write_file_if_necessary(
    path: impl AsRef<std::path::Path>,
    content: &[u8],
) -> std::io::Result<()> {
    if let Ok(cur_bytes) = std::fs::read(&path)
        && cur_bytes == content
    {
        return Ok(());
    }

    std::fs::write(path, content)
}

/// Track any files that are part of the given crate, identified by the manifest path.
fn track_crate_files(manifest_path: &Utf8Path, files_to_watch: &mut HashSet<PathBuf>) {
    let mut dep_path = manifest_path.to_owned();
    dep_path.pop();

    rerun_if_changed_glob(dep_path.join("Cargo.toml"), files_to_watch); // manifest too!
    rerun_if_changed_glob(dep_path.join("**/*.rs"), files_to_watch);
    rerun_if_changed_glob(dep_path.join("**/*.wgsl"), files_to_watch);
}

// ---

pub struct Packages<'a> {
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
        let Some(pkg) = self.pkgs.get(pkg_name) else {
            let found_names: Vec<&str> = self.pkgs.values().map(|pkg| pkg.name.as_str()).collect();
            println!("cargo::error=Failed to find package {pkg_name:?} among {found_names:?}");
            return;
        };

        // Track the root package itself
        track_crate_files(&pkg.manifest_path, files_to_watch);

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
                track_crate_files(&dep_pkg.manifest_path, files_to_watch);
            }

            if tracked.insert(dep_pkg.id.clone()) {
                self.track_patched_deps(tracked, dep_pkg, files_to_watch);
            }
        }
    }
}
