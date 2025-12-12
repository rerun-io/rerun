#![expect(clippy::unwrap_used)]
#![allow(
    clippy::allow_attributes,
    clippy::disallowed_methods,
    clippy::disallowed_types
)] // False positives for using files on Wasm
#![warn(missing_docs)]

//! This crate is to be used from `build.rs` build scripts.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;

mod git;
mod hashing;
mod rebuild_detector;
mod rustfmt;

pub use self::git::{git_branch, git_commit_hash, git_commit_short_hash};
pub use self::hashing::{
    compute_crate_hash, compute_dir_filtered_hash, compute_dir_hash, compute_file_hash,
    compute_strings_hash, iter_dir, read_versioning_hash, write_versioning_hash,
};
pub(crate) use self::rebuild_detector::Packages;
pub use self::rebuild_detector::{
    get_and_track_env_var, is_tracked_env_var_set, rebuild_if_crate_changed, rerun_if_changed,
    rerun_if_changed_glob, rerun_if_changed_or_doesnt_exist, write_file_if_necessary,
};
pub use self::rustfmt::rustfmt_str;

// ------------------

/// Should we export the build datetime for developers in the workspace?
///
/// It will be visible in analytics, in the viewer's about-menu, and with `rerun --version`.
///
/// To do so accurately may incur unnecessary recompiles, so only turn this on if you really need it.
const EXPORT_BUILD_TIME_FOR_DEVELOPERS: bool = false;

/// Should we export the current git hash/branch for developers in the workspace?
///
/// It will be visible in analytics, in the viewer's about-menu, and with `rerun --version`.
///
/// To do so accurately may incur unnecessary recompiles, so only turn this on if you really need it.
const EXPORT_GIT_FOR_DEVELOPERS: bool = false;

// ------------------

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

// ------------------

/// Where is this `build.rs` build script running?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    /// We are running `cargo publish` (via `scripts/ci/crates.py`); _probably_ on CI.
    PublishingCrates,

    /// We are running on CI for the Rerun workspace, but NOT publishing crates.
    RerunCI,

    /// We are running in the conda build environment.
    ///
    /// This is a particularly special build environment because the branch checked out is
    /// from the conda feed-stock and the build happens via source downloaded from the
    /// github-hosted tgz.
    ///
    /// See <https://github.com/conda-forge/rerun-sdk-feedstock>.
    CondaBuild,

    /// Are we a developer running inside the workspace of <https://github.com/rerun-io/rerun> ?
    DeveloperInWorkspace,

    /// We are not on Rerun's CI, and not in the Rerun workspace.
    ///
    /// This is _most likely_ a Rerun user who is compiling a `re_` crate
    /// because they depend on it either directly or indirectly in their `Cargo.toml`,
    /// or they running `cargo install rerun-cli --locked` or other tool that depend on a `re_` crate.
    ///
    /// In these cases we should do as little shenanigans in the `build.rs` as possible.
    UsedAsDependency,
}

impl Environment {
    /// Detect what environment we are running in.
    pub fn detect() -> Self {
        let is_in_rerun_workspace = is_tracked_env_var_set("IS_IN_RERUN_WORKSPACE");

        if is_tracked_env_var_set("RERUN_IS_PUBLISHING_CRATES") {
            // "RERUN_IS_PUBLISHING_CRATES" is set by `scripts/ci/crates.py`
            eprintln!("Environment: env-var RERUN_IS_PUBLISHING_CRATES is set");
            Self::PublishingCrates
        } else if is_in_rerun_workspace && std::env::var("CI").is_ok() {
            // `CI` is an env-var set by GitHub actions.
            eprintln!("Environment: env-var IS_IN_RERUN_WORKSPACE and CI are set");
            Self::RerunCI
        } else if std::env::var("CONDA_BUILD").is_ok() {
            // `CONDA_BUILD` is an env-var set by conda build
            eprintln!("Environment: env-var CONDA_BUILD is set");
            Self::CondaBuild
        } else if is_in_rerun_workspace {
            // IS_IN_RERUN_WORKSPACE is set by `.cargo/config.toml` and also in the Rust-analyzer settings in `.vscode/settings.json`
            eprintln!("Environment: env-var IS_IN_RERUN_WORKSPACE is set");
            Self::DeveloperInWorkspace
        } else {
            eprintln!("Environment: Not on CI and not in workspace");
            Self::UsedAsDependency
        }
    }
}

/// Call from the `build.rs` file of any crate you want to generate build info for.
///
/// Use this crate together with the `re_build_info` crate.
pub fn export_build_info_vars_for_crate(crate_name: &str) {
    let environment = Environment::detect();

    let export_datetime = match environment {
        Environment::PublishingCrates | Environment::RerunCI | Environment::CondaBuild => true,

        Environment::DeveloperInWorkspace => EXPORT_BUILD_TIME_FOR_DEVELOPERS,

        // Datetime won't always be accurate unless we rebuild as soon as a dependency changes,
        // and we don't want to add that burden to our users.
        Environment::UsedAsDependency => false,
    };

    let export_git_info = match environment {
        Environment::PublishingCrates | Environment::RerunCI => true,

        Environment::DeveloperInWorkspace => EXPORT_GIT_FOR_DEVELOPERS,

        // We shouldn't show the users git hash/branch in the rerun viewer.
        // TODO(jleibs): Conda builds run off a downloaded source tar-ball
        // the git environment is from conda itself.
        Environment::UsedAsDependency | Environment::CondaBuild => false,
    };

    if export_datetime && is_tracked_env_var_set("DATETIME") {
        // set externally:
        set_env(
            "RE_BUILD_DATETIME",
            &std::env::var("DATETIME").unwrap_or_default(),
        );
    } else if export_datetime {
        set_env("RE_BUILD_DATETIME", &date_time());

        // The only way to make sure the build datetime is up-to-date is to run
        // `build.rs` on every build, and there is really no good way of doing
        // so except to manually check if any files have changed:
        rebuild_if_crate_changed(crate_name);
    } else {
        set_env("RE_BUILD_DATETIME", "");
    }

    if export_git_info && is_tracked_env_var_set("GIT_HASH") && is_tracked_env_var_set("GIT_BRANCH")
    {
        // set externally:
        set_env(
            "RE_BUILD_GIT_HASH",
            &std::env::var("GIT_HASH").unwrap_or_default(),
        );
        set_env(
            "RE_BUILD_GIT_BRANCH",
            &std::env::var("GIT_BRANCH").unwrap_or_default(),
        );
    } else if export_git_info {
        set_env(
            "RE_BUILD_GIT_HASH",
            &git::git_commit_hash().unwrap_or_default(),
        );
        set_env(
            "RE_BUILD_GIT_BRANCH",
            &git::git_branch().unwrap_or_default(),
        );

        // Make sure the above are up-to-date
        git::rebuild_if_branch_or_commit_changes();
    } else {
        set_env("RE_BUILD_GIT_HASH", "");
        set_env("RE_BUILD_GIT_BRANCH", "");
    }

    // Stuff that doesn't change, so doesn't need rebuilding:
    {
        // target triple
        set_env("RE_BUILD_TARGET_TRIPLE", &std::env::var("TARGET").unwrap());

        // rust version
        let (rustc, llvm) = rust_llvm_versions().unwrap_or_default();
        set_env("RE_BUILD_RUSTC_VERSION", &rustc);
        set_env("RE_BUILD_LLVM_VERSION", &llvm);

        // We need to check `IS_IN_RERUN_WORKSPACE` in the build-script (here),
        // because otherwise it won't show up when compiling through maturin.
        // We must also make an exception for when we build actual wheels (on CI) for release.
        if environment == Environment::RerunCI {
            // e.g. building wheels on CI.
            set_env("RE_BUILD_IS_IN_RERUN_WORKSPACE", "no");
        } else {
            set_env(
                "RE_BUILD_IS_IN_RERUN_WORKSPACE",
                &std::env::var("IS_IN_RERUN_WORKSPACE").unwrap_or_default(),
            );
        }
    }

    if environment == Environment::PublishingCrates {
        // We can't query this during `cargo publish`, but we also don't need the info.
        set_env("RE_BUILD_FEATURES", "<unknown>");
    } else {
        let features = enabled_features_of(crate_name);
        let features = match features {
            Ok(features) => features.join(" "),

            // When building as a dependency on users' end, feature flag collection can fail for a
            // bunch of reasons (e.g. there's no `cargo` to begin with (Bazel, Buck, etc)).
            // Failing the build entirely is a bit too harsh in that case, everything will still
            // work just fine otherwise.
            Err(_err) if environment == Environment::UsedAsDependency => "<error>".to_owned(),

            Err(err) => panic!("{err}"),
        };

        set_env("RE_BUILD_FEATURES", &features);
    }
}

/// Jiff directly gives IOS 8601 / RFC 3339 format
fn date_time() -> String {
    jiff::Timestamp::now().to_string()
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

/// Returns `(rustc, LLVM)` versions.
///
/// Defaults to `"unknown"` if, for whatever reason, the output from `rustc -vV` did not contain
/// version information and/or the output format underwent breaking changes.
fn rust_llvm_versions() -> anyhow::Result<(String, String)> {
    let cmd = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
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

/// Returns info parsed from an invocation of the `cargo metadata` command.
///
/// You may not run this during crate publishing.
pub fn cargo_metadata() -> anyhow::Result<cargo_metadata::Metadata> {
    // See https://github.com/rerun-io/rerun/pull/7885
    anyhow::ensure!(
        Environment::detect() != Environment::PublishingCrates,
        "Can't get metadata during crate publishing - it would create a Cargo.lock file"
    );

    Ok(cargo_metadata::MetadataCommand::new()
        .no_deps()
        // Make sure this works without a connection, since docs.rs won't have one either.
        // See https://github.com/rerun-io/rerun/issues/8165
        .other_options(vec!["--frozen".to_owned()])
        .exec()?)
}

/// Returns a list of all the enabled features of the given package.
///
/// You may not run this during crate publishing.
pub fn enabled_features_of(crate_name: &str) -> anyhow::Result<Vec<String>> {
    let metadata = cargo_metadata()?;

    let mut features = vec![];
    for package in &metadata.packages {
        if package.name.as_str() == crate_name {
            for feature in package.features.keys() {
                println!("Checking if feature is enabled: {feature:?}");
                let feature_in_screaming_snake_case =
                    feature.to_ascii_uppercase().replace('-', "_");
                if std::env::var(format!("CARGO_FEATURE_{feature_in_screaming_snake_case}")).is_ok()
                {
                    features.push(feature.clone());
                }
            }
        }
    }

    Ok(features)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_time_format() {
        // Get a timestamp string
        let timestamp = date_time();

        // Check it matches the expected format: YYYY-MM-DDThh:mm:ssZ
        // This regex checks for the ISO 8601 / RFC 3339 format
        let regex =
            regex_lite::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$").unwrap();
        assert!(
            regex.is_match(&timestamp),
            "Timestamp format is incorrect: {timestamp}"
        );
    }
}
