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

fn main() {
    // target triple
    println!(
        "cargo:rustc-env=__RERUN_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );

    match git_hash() {
        Ok(git_hash) => {
            println!("cargo:rustc-env=__RERUN_GIT_HASH={git_hash}");
            for path in glob::glob("../../.git/refs/heads/**").unwrap() {
                println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
            }
        }
        Err(_) => println!("cargo:rustc-env=__RERUN_GIT_HASH=<unknown>"),
    }
}

fn git_hash() -> anyhow::Result<String> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;

    let git_hash = String::from_utf8(output.stdout)?;
    let git_hash = git_hash.trim();
    if git_hash.is_empty() {
        anyhow::bail!("empty commit hash");
    }

    let clean = Command::new("git")
        .args(["diff-files", "--quiet"])
        .output()?
        .status
        .success();

    Ok(format!("{}{}", git_hash, if clean { "" } else { "-dirty" }))
}
