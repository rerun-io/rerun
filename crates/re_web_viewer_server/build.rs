use std::collections::{HashMap, HashSet};

use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package, PackageId};

// ---

// Mapping to cargo:rerun-if-changed with glob support
fn rerun_if_changed(path: &str) {
    // Workaround for windows verbatim paths not working with glob.
    // Issue: https://github.com/rust-lang/glob/issues/111
    // Fix: https://github.com/rust-lang/glob/pull/112
    // Fixed on upstream, but no release containing the fix as of writing.
    let path = path.trim_start_matches(r"\\?\");

    for path in glob::glob(path).unwrap() {
        println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
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
    pub fn track_implicit_dep(&self, pkg_name: &str) {
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
            rerun_if_changed(path.join("Cargo.toml").as_ref());
            rerun_if_changed(path.join("**/*.rs").as_ref());
        }

        // Track all direct and indirect dependencies of that root package
        let mut tracked = HashSet::new();
        self.track_patched_deps(&mut tracked, pkg);
    }

    /// Recursively walk the tree of dependencies of the given `root` package, making sure
    /// to track all potentially modified, locally patched dependencies.
    fn track_patched_deps(&self, tracked: &mut HashSet<PackageId>, root: &Package) {
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

                rerun_if_changed(dep_path.join("Cargo.toml").as_ref()); // manifest too!
                rerun_if_changed(dep_path.join("**/*.rs").as_ref());
            }

            if tracked.insert(dep_pkg.id.clone()) {
                self.track_patched_deps(tracked, dep_pkg);
            }
        }
    }
}

fn main() {
    if std::env::var("IS_IN_RERUN_WORKSPACE") != Ok("yes".to_owned()) {
        // Only run if we are in the rerun workspace, not on users machines.
        return;
    }
    if std::env::var("RERUN_IS_PUBLISHING") == Ok("yes".to_owned()) {
        // We don't need to rebuild - we should have done so beforehand!
        // See `RELEASES.md`
        return;
    }

    // Rebuild the web-viewer Wasm,
    // because the web_server library bundles it with `include_bytes!`

    let metadata = MetadataCommand::new()
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    rerun_if_changed("../../web_viewer/favicon.ico");
    rerun_if_changed("../../web_viewer/index.html");
    rerun_if_changed("../../web_viewer/sw.js");

    let pkgs = Packages::from_metadata(&metadata);
    // We implicitly depend on re_viewer, which means we also implicitly depend on
    // all of its direct and indirect dependencies (which are potentially in-workspace
    // or patched!).
    pkgs.track_implicit_dep("re_viewer");

    if std::env::var("CARGO_FEATURE___CI").is_ok() {
        // If the `__ci` feature is set we skip building the web viewer wasm, saving a lot of time.
        // This feature is set on CI (hence the name), but also with `--all-features`, which is set by rust analyzer, bacon, etc.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
    } else {
        let release = std::env::var("PROFILE").unwrap() == "release";
        re_build_web_viewer::build(release);
    }
}
