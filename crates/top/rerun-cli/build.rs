fn main() {
    re_build_tools::export_build_info_vars_for_crate("rerun-cli");

    // Warn about not using the `nasm` feature in a release build.
    let is_release = std::env::var("PROFILE").unwrap() == "release";
    let has_nasm_feature = std::env::var("CARGO_FEATURE_NASM").is_ok();
    if is_release && !has_nasm_feature {
        println!(
            "cargo:warning=Rerun is compiled in release mode without the `nasm` feature activated. \
            Enabling the `nasm` feature is recommended for better video decoding performance. \
            This requires that the `nasm` CLI is installed and available in the current PATH."
        );
    }
}
