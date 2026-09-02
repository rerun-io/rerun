fn main() {
    re_build_tools::export_build_info_vars_for_crate("re_viewer");

    let target_os = re_build_tools::get_and_track_env_var("CARGO_CFG_TARGET_OS");
    let target_arch = re_build_tools::get_and_track_env_var("CARGO_CFG_TARGET_ARCH");
    if target_os.as_deref() == Ok("linux") && target_arch.as_deref() == Ok("aarch64") {
        // `re_rav1d` contains hand-written AArch64 assembly that refers to globals such as
        // `dav1d_mc_warp_filter` using direct, page-relative relocations. When producing a shared
        // object, symbols with default visibility may be replaced ("preempted") by definitions
        // from another shared object at runtime. The linker therefore cannot assume that such a
        // symbol will remain close enough for the assembly's direct relocation and rejects the
        // link with an `R_AARCH64_ADR_PREL_PG_HI21`/"recompile with -fPIC" error.
        //
        // An ELF linker version script can mark selected symbols as local to the shared object.
        // Local `dav1d_*` symbols cannot be preempted, so the linker knows that the direct
        // relocations are safe. Symbols not matched by this wildcard retain their normal
        // visibility. This is intentionally narrower than `--exclude-libs,ALL`, which would hide
        // symbols extracted from every static archive participating in the link.
        //
        // The script is generated in `OUT_DIR` because it is a build artifact. `-Wl,` tells the
        // compiler's linker driver to pass `--version-script=…` to the ELF linker. The Cargo
        // directive applies only when this package is linked as a `cdylib`; it is not embedded in
        // the `re_viewer` rlib and does not propagate to downstream shared libraries.
        let linker_script =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR should be set"))
                .join("hide-rav1d-symbols.map");
        std::fs::write(&linker_script, "{\n    local:\n        dav1d_*;\n};\n")
            .expect("failed to write rav1d linker version script");

        println!(
            "cargo::rustc-link-arg-cdylib=-Wl,--version-script={}",
            linker_script.display()
        );
    }
}
