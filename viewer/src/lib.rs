//! rerun viewer.

mod app;
#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
pub(crate) mod context_panel;
pub(crate) mod log_db;
pub(crate) mod log_table_view;
pub(crate) mod mesh_loader;
pub(crate) mod misc;
mod remote_viewer_app;
pub(crate) mod space_view;
pub(crate) mod time_axis;
mod time_control;
pub(crate) mod time_panel;
pub(crate) mod view_2d;
pub(crate) mod view_3d;
mod viewer_context;

pub(crate) use log_db::LogDb;
pub(crate) use time_control::TimeControl;
pub(crate) use viewer_context::{Selection, ViewerContext};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

pub use app::App;
pub use remote_viewer_app::RemoteViewerApp;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Preview {
    Small,
    Medium,
    Specific(f32),
}

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
