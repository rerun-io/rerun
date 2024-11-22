fn main() {
    // https://blog.rust-lang.org/2024/05/06/check-cfg.html
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");
}
