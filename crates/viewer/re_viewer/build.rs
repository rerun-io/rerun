fn main() {
    re_build_tools::export_build_info_vars_for_crate("re_viewer");

    cfg_aliases::cfg_aliases! {
        desktop: { all(not(target_arch = "wasm32"), not(target_os = "android")) }
    }
}
