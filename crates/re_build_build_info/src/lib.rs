#![allow(clippy::unwrap_used)]

//! This crate is to be used from `build.rs` build scripts.
//!
//! Use this crate together with the `re_build_info` crate.

use anyhow::Context as _;

use std::process::Command;

mod rebuild_detector;

pub use rebuild_detector::rebuild_if_crate_changed;

// Situations to consider
// ----------------------
//
// # Using the published crate
//
// The published crate carries its version around, which in turns gives us the git tag, which makes
// the commit hash irrelevant.
// We still need to compute _something_ so that we can actually build, but that value will be
// ignored when the crate is built by the end user anyhow.
//
// # Working directly within the workspace
//
// When working within the workspace, we can simply try and call `git` and we're done.
//
// # Using an unpublished crate (e.g. `path = "..."` or `git = "..."` or `[patch.crates-io]`)
//
// In these cases we may or may not have access to the workspace (e.g. a `path = ...` import likely
// will, while a crate patch won't).
//
// This is not an issue however, as we can simply try and see what we get.
// If we manage to compute a commit hash, great, otherwise we still have the crate version to
// fallback on.

/// Call from the `build.rs` file of any crate you want to generate build info for.
pub fn export_env_vars() {
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
    if std::env::var("CI").is_ok() {
        // Probably building wheels on CI.
        // `CI` is an env-var set by GitHub actions.
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
                    rerun_if_changed(&path); // Track changes to commit hash
                }
            }
        }
    }
}

fn rerun_if_changed(path: &str) {
    // Make sure the file exists, otherwise we'll be rebuilding all the time.
    assert!(
        std::path::Path::new(path).exists(),
        "Failed to find {path:?}"
    );
    println!("cargo:rerun-if-changed={path}");
}

fn set_env(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
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
fn git_path(path: &str) -> anyhow::Result<String> {
    run_command("git", &["rev-parse", "--git-path", path])
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
