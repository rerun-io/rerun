//! rerun viewer.

mod app;
pub mod math;
mod misc;
mod remote_viewer_app;
mod ui;

pub(crate) use misc::*;
pub(crate) use ui::*;

pub use app::App;
pub use remote_viewer_app::RemoteViewerApp;

// ----------------------------------------------------------------------------
// When compiling for native:

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub use misc::profiler::Profiler;

// ----------------------------------------------------------------------------
// When compiling for web:

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::start;

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_scope!($($arg)*);
    };
}

// ---------------------------------------------------------------------------

pub(crate) fn customize_egui(_ctx: &egui::Context) {
    // perhaps we want to use a different font, or change the theme
}
