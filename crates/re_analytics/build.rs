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
    println!("cargo:rustc-env=__RERUN_GIT_HASH={}", git_hash);
}
