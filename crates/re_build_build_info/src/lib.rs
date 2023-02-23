//! This crate is to be used from `build.rs` build scripts.
//!
//! Use this crate together with the `re_build_info` crate.

use anyhow::Context as _;

use std::process::Command;

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
    println!(
        "cargo:rustc-env=RE_BUILD_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );

    if let Ok(git_hash) = git_hash() {
        println!("cargo:rustc-env=RE_BUILD_GIT_HASH={git_hash}");
        for path in glob::glob("../../.git/refs/heads/**").unwrap() {
            println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
        }
    } else {
        // NOTE: In 99% of cases, if `git_hash` failed it's because we're not in a git repository
        // to begin with, which happens because we've imported the published crate from crates.io.
        //
        // Example of unlikely cases where the above does not hold:
        // - `git` is not installed
        // - the user downloaded rerun as a tarball and then imported via a `path = ...` import
        // - others?
        println!("cargo:rustc-env=RE_BUILD_GIT_HASH=");
    }

    let git_branch = git_branch().unwrap_or_default();
    println!("cargo:rustc-env=RE_BUILD_GIT_BRANCH={git_branch}");

    let is_git_clean = is_git_clean().unwrap_or_default();
    println!("cargo:rustc-env=RE_BUILD_GIT_IS_CLEAN={is_git_clean}");

    let time_format =
        time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z").unwrap();
    let date_time = time::OffsetDateTime::now_utc()
        .format(&time_format)
        .unwrap();
    println!("cargo:rustc-env=RE_BUILD_DATETIME={date_time}");
}

fn run_command(cmd: &'static str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("running '{cmd}'"))?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn git_hash() -> anyhow::Result<String> {
    let git_hash = run_command("git", &["rev-parse", "HEAD"])?;
    if git_hash.is_empty() {
        anyhow::bail!("empty commit hash");
    }
    let clean = is_git_clean()?;
    Ok(format!("{}{}", git_hash, if clean { "" } else { "-dirty" }))
}

fn is_git_clean() -> anyhow::Result<bool> {
    Ok(Command::new("git")
        .args(["diff-files", "--quiet"])
        .output()?
        .status
        .success())
}

fn git_branch() -> anyhow::Result<String> {
    run_command("git", &["symbolic-ref", "--short", "HEAD"])
}
