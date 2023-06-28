//! The Rerun C SDK.
//!
//! The functions here must match `rerun.h`.

#![crate_type = "staticlib"]

// SAFETY: the unsafety comes from #[no_mangle], because we can declare multiple
// functions with the same symbol names, and the linker behavior in this case i undefined.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rerun_print_hello_world() {
    println!("Hello from Rust!");
}
