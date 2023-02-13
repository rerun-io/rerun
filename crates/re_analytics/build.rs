use std::process::Command;

fn main() {
    // target triple
    println!(
        "cargo:rustc-env=__RERUN_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );

    if std::env::var("IS_IN_RERUN_WORKSPACE") != Ok("yes".to_owned()) {
        // If we're outside the workspace, we just can't know... but we still need to set the
        // envvar to the something, else we wouldn't be able to compile.
        println!("cargo:rustc-env=__RERUN_GIT_HASH=<unknown>",);
        return;
    }

    // git hash
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    let git_hash = git_hash.trim();
    let clean = Command::new("git")
        .args(["diff-files", "--quiet"])
        .output()
        .unwrap()
        .status
        .success();
    println!(
        "cargo:rustc-env=__RERUN_GIT_HASH={}{}",
        git_hash,
        if clean { "" } else { "-dirty" }
    );
    for path in glob::glob("../../.git/refs/heads/**").unwrap() {
        println!("cargo:rerun-if-changed={}", path.unwrap().to_string_lossy());
    }
}
