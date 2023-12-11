//! Helpers for tracing/spans/flamegraphs and such.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "server")]
mod server;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "server")]
pub use server::Profiler;

pub mod reexports {
    #[cfg(not(target_arch = "wasm32"))]
    pub use puffin;
}

/// Create a profile scope based on the function name.
///
/// Call this at the very top of an expensive function.
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_function!($($arg)*);
    };
}

/// Create a profiling scope with a custom name.
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope!($($arg)*);
    };
}

/// Create a special profiling scope that indicates that we are waiting
/// for some other thread to finish.
///
/// You should pass in the name of the thing you are waiting for as the first argument.
///
/// # Example
/// ```ignore
/// let normals = {
///     profile_wait!("compute_normals");
///     things.par_iter().for_each(compute_normals)
/// };
/// ```
#[macro_export]
macro_rules! profile_wait {
    () => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope!("[WAIT]");
    };
    ($id:expr) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope!(concat!("[WAIT] ", $id));
    };
    ($id:expr, $data:expr) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope!(concat!("[WAIT] ", $id), $data);
    };
}
