//! # Situations to consider regarding git
//!
//! ## Using the published crate
//!
//! The published crate carries its version around, which in turns gives us the git tag, which makes
//! the commit hash irrelevant.
//! We still need to compute _something_ so that we can actually build, but that value will be
//! ignored when the crate is built by the end user anyhow.
//!
//! ## Working directly within the workspace
//!
//! When working within the workspace, we can simply try and call `git` and we're done.
//!
//! ## Using an unpublished crate (e.g. `path = "…"` or `git = "…"` or `[patch.crates-io]`)
//!
//! In these cases we may or may not have access to the workspace (e.g. a `path = …` import likely
//! will, while a crate patch won't).
//!
//! This is not an issue however, as we can simply try and see what we get.
//! If we manage to compute a commit hash, great, otherwise we still have the crate version to
//! fallback on.

use std::path::PathBuf;

use crate::{rerun_if_changed, run_command};

pub fn rebuild_if_branch_or_commit_changes() {
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

/// Get the full commit hash
pub fn git_commit_hash() -> anyhow::Result<String> {
    let git_hash = run_command("git", &["rev-parse", "HEAD"])?;
    if git_hash.is_empty() {
        anyhow::bail!("empty commit hash");
    }
    Ok(git_hash)
}

/// Get the first 7 characters of the commit hash
pub fn git_commit_short_hash() -> anyhow::Result<String> {
    Ok(git_commit_hash()?[0..7].to_string())
}

fn parse_branch_from_github_ref() -> Option<String> {
    // The ref given is fully-formed, meaning that for branches the format is refs/heads/<branch_name>,
    // for pull requests it is refs/pull/<pr_number>/merge, and for tags it is refs/tags/<tag_name>.
    // For example, refs/heads/feature-branch-1.
    // https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables

    let github_ref = std::env::var("GITHUB_REF").ok()?;
    let branch = github_ref
        .strip_prefix("refs/")?
        .split_once('/')?
        .1
        .to_owned();
    Some(branch)
}

/// Get the current git branch name
pub fn git_branch() -> anyhow::Result<String> {
    if let Some(branch) = parse_branch_from_github_ref() {
        Ok(branch)
    } else {
        run_command("git", &["symbolic-ref", "--short", "HEAD"])
    }
}

/// From <https://git-scm.com/docs/git-rev-parse>:
///
/// Resolve `$GIT_DIR/<path>` and takes other path relocation variables such as `$GIT_OBJECT_DIRECTORY`, `$GIT_INDEX_FILE…​` into account.
/// For example, if `$GIT_OBJECT_DIRECTORY` is set to /foo/bar then `git rev-parse --git-path objects/abc` returns `/foo/bar/abc`.
fn git_path(path: &str) -> anyhow::Result<PathBuf> {
    let path = run_command("git", &["rev-parse", "--git-path", path])?;
    Ok(path.into())
}
