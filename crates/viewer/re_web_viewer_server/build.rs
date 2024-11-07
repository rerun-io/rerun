fn main() {
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");
}
