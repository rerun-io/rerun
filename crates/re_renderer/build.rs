//! This build script implements the second half of our cross-platform shader #import system.
//! The first half can be found in `src/file_resolver.rs`.
//!
//! It finds all WGSL shaders defined anywhere within `re_renderer`, and embeds them
//! directly into the released artifact for our `re_renderer` library.
//!
//! At run-time, for release builds only, those shaders will be available through an hermetic
//! virtual filesystem.
//! To the user, it will look like business as usual.
//!
//! See `re_renderer/src/workspace_shaders.rs` for the end result.

// TODO(cmc): this should only run for release builds

#![allow(clippy::unwrap_used)]

use std::path::Path;

use walkdir::{DirEntry, WalkDir};

// ---

fn rerun_if_changed(path: &std::path::Path) {
    // Make sure the file exists, otherwise we'll be rebuilding all the time.
    assert!(path.exists(), "Failed to find {path:?}");
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

// ---

use std::path::PathBuf;

use anyhow::{bail, ensure, Context as _};

/// A pre-parsed import clause, as in `#import <something>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportClause {
    /// The path being imported, as-is: neither canonicalized nor normalized.
    path: PathBuf,
}

impl ImportClause {
    pub const PREFIX: &str = "#import ";
}

impl<P: Into<PathBuf>> From<P> for ImportClause {
    fn from(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl std::str::FromStr for ImportClause {
    type Err = anyhow::Error;

    fn from_str(clause_str: &str) -> Result<Self, Self::Err> {
        let s = clause_str.trim();

        ensure!(
            s.starts_with(ImportClause::PREFIX),
            "import clause must start with {prefix:?}, got {s:?}",
            prefix = ImportClause::PREFIX,
        );
        let s = s.trim_start_matches(ImportClause::PREFIX).trim();

        let rs = s.chars().rev().collect::<String>();

        let splits = s
            .find('<')
            .and_then(|i0| rs.find('>').map(|i1| (i0 + 1, rs.len() - i1 - 1)));

        if let Some((i0, i1)) = splits {
            let s = &s[i0..i1];
            ensure!(!s.is_empty(), "import clause must contain a non-empty path");

            return s
                .parse()
                .with_context(|| "couldn't parse {s:?} as PathBuf")
                .map(|path| Self { path });
        }

        bail!("malformed import clause: {clause_str:?}")
    }
}

fn check_hermeticity(root_path: impl AsRef<Path>, file_path: impl AsRef<Path>) {
    let file_path = file_path.as_ref();
    let dir_path = file_path.parent().unwrap();
    std::fs::read_to_string(file_path)
        .unwrap()
        .lines()
        .try_for_each(|line| {
            if !line.trim().starts_with(ImportClause::PREFIX) {
                return Ok(());
            }

            let clause = line.parse::<ImportClause>()?;
            let clause_path = dir_path.join(clause.path);
            let clause_path = std::fs::canonicalize(clause_path)?;
            ensure!(
                clause_path.starts_with(&root_path),
                "trying to import {:?} which lives outside of the workspace, \
                    this is illegal in release and/or Wasm builds!",
                clause_path
            );

            Ok::<_, anyhow::Error>(())
        })
        .unwrap();
}

// ---

fn main() {
    if std::env::var("CI").is_ok() {
        // Don't run on CI!
        //
        // The code we're generating here is actual source code that gets committed into the
        // repository.
        return;
    }
    if std::env::var("IS_IN_RERUN_WORKSPACE") != Ok("yes".to_owned()) {
        // Only run if we are in the rerun workspace, not on users machines.
        return;
    }
    if std::env::var("RERUN_IS_PUBLISHING") == Ok("yes".to_owned()) {
        // We don't need to rebuild - we should have done so beforehand!
        // See `RELEASES.md`
        return;
    }

    // Root path of the re_renderer crate.
    //
    // We're packing at that level rather than at the workspace level because we lose all workspace
    // layout information when publishing the crates.
    // This means all the shaders we pack must live under `re_renderer/shader` for now.
    let manifest_path = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).to_owned();
    let shader_dir = manifest_path.join("shader");

    // On windows at least, it's been shown that the paths we get out of these env-vars can
    // actually turn out _not_ to be canonicalized in practice, which of course will break
    // hermeticity checks later down the line.
    //
    // So: canonicalize them all, just in case... ¯\_(ツ)_/¯
    let manifest_path = std::fs::canonicalize(manifest_path).unwrap();
    let shader_dir = std::fs::canonicalize(shader_dir).unwrap();

    let src_path = manifest_path.join("src");
    let file_path = src_path.join("workspace_shaders.rs");

    fn is_wgsl_or_dir(entry: &DirEntry) -> bool {
        let is_dir = entry.file_type().is_dir();
        let is_wgsl = entry
            .file_name()
            .to_str()
            .map_or(false, |s| s.ends_with(".wgsl"));
        is_dir || is_wgsl
    }

    // We do our best to generate code that passes rustfmt, even though we also
    // add `#[rustfmt::skip]` to the whole module.

    let mut contents = r#"// This file is autogenerated via build.rs.
// DO NOT EDIT.

use std::path::Path;

static ONCE: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false);

pub fn init() {
    if ONCE.swap(true, ::std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    use crate::file_system::FileSystem as _;
    let fs = crate::MemFileSystem::get();
"#
    .to_owned();

    let walker = WalkDir::new(&shader_dir).into_iter();
    let entries = {
        let mut entries = walker
            .filter_entry(is_wgsl_or_dir)
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.path().cmp(b.path()));
        entries
    };

    assert!(
        !entries.is_empty(),
        "re_renderer build.rs found no shaders - I think some path is wrong!"
    );

    for entry in entries {
        rerun_if_changed(entry.path());

        // The relative path to get from the current shader file to `workspace_shaders.rs`.
        // We must make sure to pass relative paths to `include_str`!
        let relpath = pathdiff::diff_paths(entry.path(), &src_path).unwrap();
        let relpath = relpath.to_str().unwrap().replace('\\', "/"); // Force slashes on Windows.

        // The hermetic path used in the virtual filesystem at run-time.
        //
        // This is using the exact same strip_prefix as the standard `file!()` macro, so that
        // hermetic paths generated by one will be comparable with the hermetic paths generated
        // by the other!
        let virtpath = entry.path().strip_prefix(&manifest_path).unwrap();
        let virtpath = virtpath.to_str().unwrap().replace('\\', "/"); // Force slashes on Windows.

        let is_release = cfg!(not(debug_assertions));
        // DO NOT USE `cfg!` for this, that would give you the host's platform!
        let targets_wasm = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap() == "wasm";

        // Make sure we're not referencing anything outside of the workspace!
        //
        // TODO(cmc): At the moment we only look for breaches of hermiticity at the import level
        // and completely ignore top-level, e.g. `#import </tmp/shader.wgsl>` will fail as
        // expected in release builds, while `include_file!("/tmp/shader.wgsl")` won't!
        //
        // The only way to make hermeticity checks work for top-level files would be to read all
        // Rust files and parse all `include_file!` statements in those, so that we actually
        // know what those external top-level files are to begin with.
        // Not worth it... for now.
        if is_release || targets_wasm {
            check_hermeticity(&manifest_path, entry.path()); // will fail if not hermetic
        }

        contents += &format!(
            "
    {{
        let virtpath = Path::new(\"{virtpath}\");
        let content = include_str!(\"{relpath}\").into();
        fs.create_file(virtpath, content).unwrap();
    }}
",
        );
    }

    contents = format!("{}\n}}\n", contents.trim_end());

    write_file_if_necessary(file_path, contents.as_bytes()).unwrap();
}

/// Only touch the file if the contents has actually changed
fn write_file_if_necessary(
    dst_path: impl AsRef<std::path::Path>,
    content: &[u8],
) -> std::io::Result<()> {
    if let Ok(cur_bytes) = std::fs::read(&dst_path) {
        if cur_bytes == content {
            return Ok(());
        }
    }

    std::fs::write(dst_path, content)
}
