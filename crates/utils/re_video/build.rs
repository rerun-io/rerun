fn main() {
    re_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));

    cfg_aliases::cfg_aliases! {
        native: { not(target_arch = "wasm32") },
        linux_arm64: { all(target_os = "linux", target_arch = "aarch64") },
        with_dav1d: { all(feature = "av1", native, not(linux_arm64)) }, // https://github.com/rerun-io/rerun/issues/7755
        with_ffmpeg: { all(feature= "ffmpeg", native) }
    }
}
