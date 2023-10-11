#![allow(clippy::unwrap_used)]

//! This crate is to be used from `build.rs` build scripts.

use anyhow::Context as _;

use std::sync::atomic::{AtomicBool, Ordering};
use std::{path::PathBuf, process::Command};

mod hashing;
mod rebuild_detector;

pub(crate) use self::rebuild_detector::Packages;

pub use self::hashing::{
    compute_crate_hash, compute_dir_filtered_hash, compute_dir_hash, compute_file_hash,
    compute_strings_hash, iter_dir, read_versioning_hash, write_versioning_hash,
};
pub use self::rebuild_detector::{
    get_and_track_env_var, is_tracked_env_var_set, rebuild_if_crate_changed, rerun_if_changed,
    rerun_if_changed_glob, rerun_if_changed_or_doesnt_exist, write_file_if_necessary,
};

/// Atomic bool indicating whether or not to print the cargo build instructions
pub(crate) static OUTPUT_CARGO_BUILD_INSTRUCTIONS: AtomicBool = AtomicBool::new(true);

/// Change whether or not this library should output cargo build instructions
pub fn set_output_cargo_build_instructions(output_instructions: bool) {
    OUTPUT_CARGO_BUILD_INSTRUCTIONS.store(output_instructions, Ordering::Relaxed);
}

/// Helper to check whether or not cargo build instructions should be printed.
pub(crate) fn should_output_cargo_build_instructions() -> bool {
    OUTPUT_CARGO_BUILD_INSTRUCTIONS.load(Ordering::Relaxed)
}

/// Where is this `build.rs` build script running?
pub enum Environment {
    /// We are running `cargo publish` (via `scripts/ci/crates.py`); _probably_ on CI.
    PublishingCrates,

    /// We are running on CI, but NOT publishing crates
    CI,

    /// Are we a developer running inside the workspace of <https://github.com/rerun-io/rerun> ?
    DeveloperInWorkspace,

    /// We are not on CI, and not in the Rerun workspace.
    ///
    /// This is _most likely_ a Rerun user who is compiling a `re_` crate
    /// because they depend on it either directly or indirectly in their `Cargo.toml`,
    /// or they running `cargo install rerun-cli` or other tool that depend on a `re_` crate.
    ///
    /// In these cases we should do as little shenanigans in the `build.rs` as possible.
    UsedAsDependency,
}

impl Environment {
    /// Detect what environment we are running in.
    pub fn detect() -> Self {
        if is_tracked_env_var_set("RERUN_IS_PUBLISHING") {
            // "RERUN_IS_PUBLISHING" is set by `scripts/ci/crates.py`
            eprintln!("Environment: env-var RERUN_IS_PUBLISHING is set");
            Self::PublishingCrates
        } else if is_on_ci() {
            // `CI` is an env-var set by GitHub actions.
            eprintln!("Environment: env-var CI is set");
            Self::CI
        } else if is_tracked_env_var_set("IS_IN_RERUN_WORKSPACE") {
            // IS_IN_RERUN_WORKSPACE is set by `.cargo/config.toml` and also in the Rust-analyzer settings in `.vscode/settings.json`
            eprintln!("Environment: env-var IS_IN_RERUN_WORKSPACE is set");
            Self::DeveloperInWorkspace
        } else {
            eprintln!("Environment: Not on CI anmd not in workspace");
            Self::UsedAsDependency
        }
    }
}

/// Are we running on a CI machine?
pub fn is_on_ci() -> bool {
    // `CI` is an env-var set by GitHub actions.
    std::env::var("CI").is_ok()
}

/// Call from the `build.rs` file of any crate you want to generate build info for.
///
/// Use this crate together with the `re_build_info` crate.
pub fn export_build_info_vars_for_crate(crate_name: &str) {
    rebuild_if_crate_changed(crate_name);
    export_build_info_env_vars();
}

/// # Situations to consider regarding git
///
/// ## Using the published crate
///
/// The published crate carries its version around, which in turns gives us the git tag, which makes
/// the commit hash irrelevant.
/// We still need to compute _something_ so that we can actually build, but that value will be
/// ignored when the crate is built by the end user anyhow.
///
/// ## Working directly within the workspace
///
/// When working within the workspace, we can simply try and call `git` and we're done.
///
/// ## Using an unpublished crate (e.g. `path = "…"` or `git = "…"` or `[patch.crates-io]`)
///
/// In these cases we may or may not have access to the workspace (e.g. a `path = …` import likely
/// will, while a crate patch won't).
///
/// This is not an issue however, as we can simply try and see what we get.
/// If we manage to compute a commit hash, great, otherwise we still have the crate version to
/// fallback on.
fn export_build_info_env_vars() {
    // target triple
    set_env("RE_BUILD_TARGET_TRIPLE", &std::env::var("TARGET").unwrap());
    set_env("RE_BUILD_GIT_HASH", &git_hash().unwrap_or_default());
    set_env("RE_BUILD_GIT_BRANCH", &git_branch().unwrap_or_default());

    // rust version
    let (rustc, llvm) = rust_version().unwrap_or_default();
    set_env("RE_BUILD_RUSTC_VERSION", &rustc);
    set_env("RE_BUILD_LLVM_VERSION", &llvm);

    // We need to check `IS_IN_RERUN_WORKSPACE` in the build-script (here),
    // because otherwise it won't show up when compiling through maturin.
    // We must also make an exception for when we build actual wheels (on CI) for release.
    if is_on_ci() {
        // e.g. building wheels on CI.
        set_env("RE_BUILD_IS_IN_RERUN_WORKSPACE", "no");
    } else {
        set_env(
            "RE_BUILD_IS_IN_RERUN_WORKSPACE",
            &std::env::var("IS_IN_RERUN_WORKSPACE").unwrap_or_default(),
        );
    }

    let time_format =
        time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z").unwrap();
    let date_time = time::OffsetDateTime::now_utc()
        .format(&time_format)
        .unwrap();
    set_env("RE_BUILD_DATETIME", &date_time);

    // Make sure we re-run the build script if the branch or commit changes:
    if let Ok(head_path) = git_path("HEAD") {
        rerun_if_changed(&head_path); // Track changes to branch
        if let Ok(head) = std::fs::read_to_string(&head_path) {
            if let Some(git_file) = head.strip_prefix("ref: ") {
                if let Ok(path) = git_path(git_file) {
                    if path.exists() {
                        rerun_if_changed(path); // Track changes to commit hash
                    } else {
                        // Weird that it doesn't exist. Maybe we will miss a git hash change,
                        // but that is better that tracking a non-existing files (which leads to constant rebuilds).
                        // See https://github.com/rerun-io/rerun/issues/2380 for more
                    }
                }
            }
        }
    }
}

fn set_env(name: &str, value: &str) {
    if should_output_cargo_build_instructions() {
        println!("cargo:rustc-env={name}={value}");
    }
}

fn run_command(cmd: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("running '{cmd}'"))?;

    anyhow::ensure!(
        output.status.success(),
        "Failed to run '{cmd} {args:?}':\n{}\n{}\n",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn git_hash() -> anyhow::Result<String> {
    let git_hash = run_command("git", &["rev-parse", "HEAD"])?;
    if git_hash.is_empty() {
        anyhow::bail!("empty commit hash");
    }
    Ok(git_hash)
}

fn git_branch() -> anyhow::Result<String> {
    run_command("git", &["symbolic-ref", "--short", "HEAD"])
}

/// From <https://git-scm.com/docs/git-rev-parse>:
///
/// Resolve `$GIT_DIR/<path>` and takes other path relocation variables such as `$GIT_OBJECT_DIRECTORY`, `$GIT_INDEX_FILE…​` into account.
/// For example, if `$GIT_OBJECT_DIRECTORY` is set to /foo/bar then `git rev-parse --git-path objects/abc` returns `/foo/bar/abc`.
fn git_path(path: &str) -> anyhow::Result<PathBuf> {
    let path = run_command("git", &["rev-parse", "--git-path", path])?;
    Ok(path.into())
}

/// Returns `(rustc, LLVM)` versions.
///
/// Defaults to `"unknown"` if, for whatever reason, the output from `rustc -vV` did not contain
/// version information and/or the output format underwent breaking changes.
fn rust_version() -> anyhow::Result<(String, String)> {
    let cmd = std::env::var("RUSTC").unwrap_or("rustc".into());
    let args = &["-vV"];

    // $ rustc -vV
    // rustc 1.67.0 (fc594f156 2023-01-24)
    // binary: rustc
    // commit-hash: fc594f15669680fa70d255faec3ca3fb507c3405
    // commit-date: 2023-01-24
    // host: x86_64-unknown-linux-gnu
    // release: 1.67.0
    // LLVM version: 15.0.6

    let res = run_command(&cmd, args)?;

    let mut rustc_version = None;
    let mut llvm_version = None;

    for line in res.lines() {
        if let Some(version) = line.strip_prefix("rustc ") {
            rustc_version = Some(version.to_owned());
        } else if let Some(version) = line.strip_prefix("LLVM version: ") {
            llvm_version = Some(version.to_owned());
        }
    }

    // NOTE: This should never happen, but if it does, we want to make sure we can differentiate
    // between "failed to invoke rustc" vs. "rustc's output did not contain any version (??)
    // and/or the output format has changed".
    Ok((
        rustc_version.unwrap_or_else(|| "unknown".to_owned()),
        llvm_version.unwrap_or_else(|| "unknown".to_owned()),
    ))
}
