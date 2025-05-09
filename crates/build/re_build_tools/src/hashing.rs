use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::Context as _;
use sha2::{Digest as _, Sha256};

use crate::{rerun_if_changed, rerun_if_changed_or_doesnt_exist};

// ---

fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{b:02x}").expect("writing to string should never fail");
    }
    s
}

/// Recursively walks the directory at `path` in filename order.
///
/// If `extensions` is specified, only files with the right extensions will be iterated.
/// Specified extensions should _not_ include the leading dot, e.g. `fbs` rather than `.fbs`.
pub fn iter_dir<'a>(
    path: impl AsRef<Path>,
    extensions: Option<&'a [&'a str]>,
) -> impl Iterator<Item = PathBuf> + 'a {
    iter_dir_filtered(path, move |entry: &Path| {
        extensions.is_none_or(|extensions| {
            extensions.iter().any(|ext| {
                entry
                    .extension()
                    .is_some_and(|ext2| *ext == ext2.to_string_lossy())
            })
        })
    })
}

/// Recursively walks the directory at `path` in filename order with an arbitrary filter.
pub fn iter_dir_filtered<'a>(
    path: impl AsRef<Path>,
    custom_filter: impl Fn(&Path) -> bool + 'a,
) -> impl Iterator<Item = PathBuf> + 'a {
    fn filter(entry: &walkdir::DirEntry, custom_filter: &impl Fn(&Path) -> bool) -> bool {
        let is_dir = entry.file_type().is_dir();
        let is_interesting = custom_filter(entry.path());
        is_dir || is_interesting
    }

    let path = path.as_ref();
    walkdir::WalkDir::new(path)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(move |entry| filter(entry, &custom_filter))
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_type().is_file().then(|| entry.into_path()))
}

/// Given a file path, computes the sha256 hash of its contents and returns an hexadecimal string
/// for it.
///
/// This will automatically emit a `rerun-if-changed` clause for the specified file.
///
/// Panics if the file doesn't exist.
pub fn compute_file_hash(path: impl AsRef<Path>) -> String {
    let mut hasher = Sha256::new();

    let path = path.as_ref();
    let mut file = fs::File::open(path)
        .with_context(|| format!("couldn't open {path:?}"))
        .unwrap();
    io::copy(&mut file, &mut hasher)
        .with_context(|| format!("couldn't copy from {path:?}"))
        .unwrap();

    rerun_if_changed(path);

    encode_hex(hasher.finalize().as_slice())
}

/// Given a directory path, computes the sha256 hash of the accumulated contents of all of its
/// files (ordered by filename), and returns an hexadecimal string for it.
///
/// This includes files in sub-directories (i.e. it's recursive).
///
/// This will automatically emit a `rerun-if-changed` clause for all the files that were hashed.
///
/// If `extensions` is specified, only files with the right extensions will be iterated.
/// Specified extensions should _not_ include the leading dot, e.g. `fbs` rather than `.fbs`.
pub fn compute_dir_hash<'a>(path: impl AsRef<Path>, extensions: Option<&'a [&'a str]>) -> String {
    let mut hasher = Sha256::new();

    let path = path.as_ref();
    for filepath in iter_dir(path, extensions) {
        let mut file = fs::File::open(&filepath)
            .with_context(|| format!("couldn't open {filepath:?}"))
            .unwrap();
        io::copy(&mut file, &mut hasher)
            .with_context(|| format!("couldn't copy from {filepath:?}"))
            .unwrap();

        rerun_if_changed(filepath);
    }

    encode_hex(hasher.finalize().as_slice())
}

/// Given a directory path, computes the sha256 hash of the accumulated contents of all of its
/// files (ordered by filename) except those failing a custom filter, and returns an hexadecimal string for it.
///
/// This includes files in sub-directories (i.e. it's recursive).
///
/// This will automatically emit a `rerun-if-changed` clause for all the files that were hashed.
pub fn compute_dir_filtered_hash<'a>(
    path: impl AsRef<Path>,
    custom_filter: impl Fn(&Path) -> bool + 'a,
) -> String {
    let mut hasher = Sha256::new();

    let path = path.as_ref();
    for filepath in iter_dir_filtered(path, custom_filter) {
        let mut file = fs::File::open(&filepath)
            .with_context(|| format!("couldn't open {filepath:?}"))
            .unwrap();
        io::copy(&mut file, &mut hasher)
            .with_context(|| format!("couldn't copy from {filepath:?}"))
            .unwrap();

        rerun_if_changed(filepath);
    }

    encode_hex(hasher.finalize().as_slice())
}

/// Given a crate name, computes the sha256 hash of its source code (ordered by filename) and
/// returns an hexadecimal string for it.
///
/// This includes the source code of all its direct and indirect dependencies.
///
/// This will automatically emit a `rerun-if-changed` clause for all the files that were hashed.
pub fn compute_crate_hash(pkg_name: impl AsRef<str>) -> String {
    use cargo_metadata::{CargoOpt, MetadataCommand};
    let metadata = MetadataCommand::new()
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let pkg_name = pkg_name.as_ref();
    let mut files = Default::default();

    let pkgs = crate::Packages::from_metadata(&metadata);
    pkgs.track_implicit_dep(pkg_name, &mut files);

    let mut files = files.into_iter().collect::<Vec<_>>();
    files.sort();

    let hashes = files.into_iter().map(compute_file_hash).collect::<Vec<_>>();
    let hashes = hashes.iter().map(|s| s.as_str()).collect::<Vec<_>>();

    compute_strings_hash(&hashes)
}

/// Given a bunch of strings, computes the sha256 hash of their contents (in the order they
/// were passed in) and returns an hexadecimal string for it.
pub fn compute_strings_hash(strs: &[&str]) -> String {
    let mut hasher = Sha256::new();

    for s in strs {
        hasher.update(s);
    }

    encode_hex(hasher.finalize().as_slice())
}

/// Writes the given `hash` at the specified `path`.
///
/// `hash` should have been computed using of the methods in this module: [`compute_file_hash`],
/// [`compute_dir_hash`], [`compute_crate_hash`].
///
/// Panics on I/O errors.
///
/// Use [`read_versioning_hash`] to read it back.
pub fn write_versioning_hash(path: impl AsRef<Path>, hash: impl AsRef<str>) {
    let path = path.as_ref();
    let hash = hash.as_ref();

    let contents = unindent::unindent(&format!(
        "
        # This is a sha256 hash for all direct and indirect dependencies of this crate's build script.
        # It can be safely removed at anytime to force the build script to run again.
        # Check out build.rs to see how it's computed.
        {hash}
        "
    ));
    std::fs::write(path, contents.trim_start())
        .with_context(|| format!("couldn't write to {path:?}"))
        .unwrap();
}

/// Reads back a versioning hash that was written with [`write_versioning_hash`].
///
/// This will automatically emit a `rerun-if-changed` clause for the specified filepath.
///
/// Returns `None` on error.
pub fn read_versioning_hash(path: impl AsRef<Path>) -> Option<String> {
    let path = path.as_ref();

    // NOTE: It's important we trigger if the file doesn't exist, as this means the user explicitly
    // deleted the versioning file, i.e. they're trying to force a rebuild.
    rerun_if_changed_or_doesnt_exist(path);

    std::fs::read_to_string(path).ok().and_then(|contents| {
        contents
            .lines()
            .find_map(|line| (!line.trim().starts_with('#')).then(|| line.trim().to_owned()))
    })
}
