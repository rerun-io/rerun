fn main() {
    // Once we bump to Rust 1.80+ this will tell the checker that this flag actually exists for releases.
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");
}
