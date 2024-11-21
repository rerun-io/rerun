//! Helpers for tracing/spans/flamegraphs and such.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "server")]
mod server;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "server")]
pub use server::Profiler;

#[cfg(feature = "superluminal")]
pub mod superluminal;

pub mod reexports {
    #[cfg(not(target_arch = "wasm32"))]
    pub use puffin;

    #[cfg(feature = "superluminal")]
    pub use superluminal_perf;
}

/// Create a profile scope based on the function name.
///
/// Call this at the very top of an expensive function.
#[macro_export]
macro_rules! profile_function {
    () => {
        $crate::profile_function_if!(true, "")
    };

    ($data:expr) => {
        $crate::profile_function_if!(true, $data);
    };
}

/// Create a profile scope based on the function name, if the given condition holds true.
///
/// Call this at the very top of a potentially expensive function.
#[cfg(not(feature = "superluminal"))]
#[macro_export]
macro_rules! profile_function_if {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_function_if!($($arg)*);
    };
}

#[cfg(feature = "superluminal")]
#[macro_export]
macro_rules! profile_function_if {
    ($condition:expr) => {
        $crate::profile_function_if!($condition, "")
    };

    ($condition:expr, $data:expr) => {
        let _superluminal_scope = {
            // Takes a static string for low overhead, so we can't make use of the data here.
            $crate::reexports::superluminal_perf::begin_event(
                $crate::reexports::puffin::current_function_name!(),
            );
            $crate::superluminal::SuperluminalEndEventOnDrop
        }; // TODO: not a function scope.
           // TODO: condition?
    };
}

/// Create a profiling scope with a custom name.
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        $crate::profile_scope_if!(true, $name, "")
    };

    ($name:expr, $data:expr) => {
        $crate::profile_scope_if!(true, $name, $data);
    };
}

/// Create a profiling scope with a custom name, if the given condition holds true.
#[cfg(not(feature = "superluminal"))]
#[macro_export]
macro_rules! profile_scope_if {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        $crate::reexports::puffin::profile_scope_if!($($arg)*);
    };
}

#[cfg(feature = "superluminal")]
#[macro_export]
macro_rules! profile_scope_if {
    ($condition:expr, $name:expr) => {
        $crate::profile_scope_if!($condition, $name, "")
    };

    ($condition:expr, $name:expr, $data:expr) => {
        let _superluminal_scope = {
            // Takes a static string for low overhead, so we can't make use of the data here.
            $crate::reexports::superluminal_perf::begin_event(stringify!($name));
            $crate::superluminal::SuperluminalEndEventOnDrop
        }; // TODO: condition?
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
