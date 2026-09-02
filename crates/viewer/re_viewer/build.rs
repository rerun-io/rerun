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
        // Hiding matching names with a linker version script is not sufficient here: GNU ld.bfd
        // validates these relocations while it is still processing the input archive, before the
        // output's dynamic symbol visibility from a version script can make the references safe.
        // `--exclude-libs,ALL` instead marks symbols originating in static archives as hidden for
        // the purpose of the static link itself. This guarantees that the rav1d globals bind to
        // their definitions in this shared object, allowing the linker to resolve the direct
        // relocations.
        //
        // `ALL` applies to every static archive in this link because Cargo gives rlibs hashed file
        // names, such as `libre_rav1d-<hash>.rlib`; GNU ld requires exact archive names and cannot
        // select that archive with a crate-name wildcard. This does not hide symbols defined by
        // the cdylib itself. Rust cdylibs also generally should not re-export implementation
        // details pulled in from their statically linked dependencies.
        //
        // `-Wl,` tells the compiler's linker driver to pass the remaining comma-separated options
        // to the ELF linker. The Cargo directive applies only when this package is linked as a
        // `cdylib`; it is not embedded in the `re_viewer` rlib and does not propagate to downstream
        // shared libraries.
        println!("cargo::rustc-link-arg-cdylib=-Wl,--exclude-libs,ALL");
    }
}
