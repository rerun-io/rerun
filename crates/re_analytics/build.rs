use std::process::Command;

fn main() {
    // target triple
    println!(
        "cargo:rustc-env=__RERUN_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );
    println!("cargo:rerun-if-changed=build.rs");

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
    println!("cargo:rerun-if-changed=.git/HEAD");
}
