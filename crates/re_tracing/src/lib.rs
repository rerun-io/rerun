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

/// Wrapper around puffin profiler on native, no-op on wasm.
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_function!($($arg)*);
    };
}

/// Wrapper around puffin profiler on native, no-op on wasm.
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope!($($arg)*);
    };
}
