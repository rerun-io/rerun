//! rerun viewer.

mod app;
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
pub use native::run_native_viewer;

// ----------------------------------------------------------------------------
// When compiling for web:

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::start;
