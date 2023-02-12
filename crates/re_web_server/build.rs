use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package, PackageId};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Stdio,
};

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
    pub fn track_implicit_dep(&self, root: &Package) {
        // Track the root package itself
        {
            let mut path = root.manifest_path.clone();
            path.pop();

            // NOTE: Since we track the cargo manifest, past this point we only need to
            // account for locally patched dependencies.
            rerun_if_changed(path.join("Cargo.toml").as_ref());
            rerun_if_changed(path.join("**/*.rs").as_ref());
        }

        // Track all direct and indirect dependencies of that root package
        let mut tracked = HashSet::new();
        self.track_patched_deps(&mut tracked, root);
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

// Port of build_web.sh
fn build_web() {
    let repository_root_dir = format!("{}/../..", std::env!("CARGO_MANIFEST_DIR"));

    let crate_name = "re_viewer";
    let build_dir = format!("{repository_root_dir}/web_viewer");

    assert!(
        Path::new(&build_dir).exists(),
        "Failed to find dir {build_dir}. CWD: {:?}, CARGO_MANIFEST_DIR: {:?}",
        std::env::current_dir(),
        std::env!("CARGO_MANIFEST_DIR")
    );

    let wasm_path = Path::new(&build_dir).join([crate_name, "_bg.wasm"].concat());
    fs::remove_file(wasm_path.clone()).ok();

    let metadata = MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let target_wasm = format!("{}_wasm", metadata.target_directory);
    let release = std::env::var("PROFILE").unwrap() == "release";

    let root_dir = metadata.target_directory.parent().unwrap();

    // --------------------------------------------------------------------------------
    // Compile rust to wasm

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(root_dir);
    cmd.args([
        "build",
        "--target-dir",
        &target_wasm,
        "-p",
        crate_name,
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]);
    cmd.env("RUSTFLAGS", "--cfg=web_sys_unstable_apis");
    cmd.env("CARGO_ENCODED_RUSTFLAGS", "");

    if release {
        cmd.arg("--release");
    }

    eprintln!("wasm build cmd: {cmd:?}");

    let output = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect("failed to compile re_viewer for wasm32");

    eprintln!("compile status: {}", output.status);
    eprintln!(
        "compile stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success());

    // --------------------------------------------------------------------------------
    // Generate JS bindings

    let build = if release { "release" } else { "debug" };

    let target_name = [crate_name, ".wasm"].concat();

    let target_path = Path::new(&target_wasm)
        .join("wasm32-unknown-unknown")
        .join(build)
        .join(target_name);

    let mut cmd = std::process::Command::new("wasm-bindgen");
    cmd.current_dir(root_dir);
    cmd.args([
        target_path.to_str().unwrap(),
        "--out-dir",
        &build_dir,
        "--no-modules",
        "--no-typescript",
    ]);

    eprintln!("wasm-bindgen cmd: {cmd:?}");

    let output = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .unwrap_or_else(|err| panic!("Failed to generate JS bindings: {err}. target_path: {target_path:?}, build_dir: {build_dir}"));

    eprintln!("wasm-bindgen status: {}", output.status);
    eprintln!(
        "wasm-bindgen stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output.status.success());

    // --------------------------------------------------------------------------------
    // Optimize the wasm

    if release {
        let wasm_path = wasm_path.to_str().unwrap();

        // to get wasm-opt:  apt/brew/dnf install binaryen
        let mut cmd = std::process::Command::new("wasm-opt");
        cmd.current_dir(root_dir);
        cmd.args([wasm_path, "-O2", "--fast-math", "-o", wasm_path]);

        eprintln!("wasm-opt cmd: {cmd:?}");

        let output = cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("failed to optimize wasm");

        eprintln!("wasm-opt status: {}", output.status);
        eprintln!(
            "wasm-opt stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(output.status.success());
    }
}

// ---

fn main() {
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
    if let Some(re_viewer) = pkgs.pkgs.get("re_viewer") {
        pkgs.track_implicit_dep(re_viewer);
    } else {
        // We are probably not in the rerun workspace,
        // but in a `cargo publish` or on a user machine.
        // No need to consider changes to `re_viewer`!
    }

    if std::env::var("CARGO_FEATURE___CI").is_ok() {
        // This saves a lot of CI time.
        eprintln!("__ci feature detected: Skipping building of web viewer wasm.");
    } else {
        eprintln!("Build web viewer wasm…");

        build_web();
    }
}
