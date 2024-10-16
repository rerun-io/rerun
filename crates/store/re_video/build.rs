fn main() {
    // uncomment these when we update to Rust 1.80: https://blog.rust-lang.org/2024/05/06/check-cfg.html
    // println!("cargo::rustc-check-cfg=cfg(native)");
    // println!("cargo::rustc-check-cfg=cfg(linux_arm64)");
    // println!("cargo::rustc-check-cfg=cfg(with_dav1d)");

    cfg_aliases::cfg_aliases! {
        native: { not(target_arch = "wasm32") },
        linux_arm64: { all(target_os = "linux", target_arch = "aarch64") },
        with_dav1d: { all(feature = "av1", native, not(linux_arm64)) }, // https://github.com/rerun-io/rerun/issues/7755
    }
}
