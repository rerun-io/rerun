//! The Rerun C SDK.
//!
//! The functions here must match `rerun.h`.

#![crate_type = "staticlib"]

use std::ffi::CString;

// SAFETY: the unsafety comes from #[no_mangle], because we can declare multiple
// functions with the same symbol names, and the linker behavior in this case i undefined.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rerun_version_string() -> *const i8 {
    use once_cell::sync::Lazy;
    static VERSION: Lazy<CString> =
        Lazy::new(|| CString::new(re_sdk::build_info().to_string()).unwrap()); // unwrap: there won't be any NUL bytes in the string

    VERSION.as_ptr()
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rerun_print_hello_world() {
    println!("Hello from Rust!");
}
