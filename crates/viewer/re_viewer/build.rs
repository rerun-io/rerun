fn main() {
    re_build_tools::export_build_info_vars_for_crate("re_viewer");

    let target_os = re_build_tools::get_and_track_env_var("CARGO_CFG_TARGET_OS");
    let target_arch = re_build_tools::get_and_track_env_var("CARGO_CFG_TARGET_ARCH");
    if target_os.as_deref() == Ok("linux") && target_arch.as_deref() == Ok("aarch64") {
        // rav1d's assembly uses direct relocations to Rust globals, which must have hidden
        // visibility in a shared library.
        println!("cargo::rustc-link-arg-cdylib=-Wl,--exclude-libs,ALL");
    }
}
