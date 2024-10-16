fn main() {
    re_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));

    // uncomment these when we update to Rust 1.80: https://blog.rust-lang.org/2024/05/06/check-cfg.html
    // println!("cargo::rustc-check-cfg=cfg(native)");
    // println!("cargo::rustc-check-cfg=cfg(linux_arm64)");
    // println!("cargo::rustc-check-cfg=cfg(with_dav1d)");

    cfg_aliases::cfg_aliases! {
        native: { not(target_arch = "wasm32") },
        linux_arm64: { all(target_os = "linux", target_arch = "arm64") },
        with_dav1d: { all(native, not(linux_arm64)) }, // https://github.com/rerun-io/rerun/issues/7755
    }
}
