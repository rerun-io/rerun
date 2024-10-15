fn main() {
    re_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));
}
