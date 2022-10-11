//! Rerun Viewer GUI.
//!
//! This crate contains all the GUI code for the Rerun Viewer,
//! including all 2D and 3D visualization code.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod app;
mod design_tokens;
pub mod math;
mod misc;
mod remote_viewer_app;
mod ui;

pub(crate) use misc::*;
pub(crate) use ui::*;

pub use app::App;
#[cfg(feature = "wgpu")]
use re_renderer::context::RenderContext;
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

pub(crate) fn customize_eframe(cc: &eframe::CreationContext<'_>) {
    #[cfg(feature = "wgpu")]
    {
        let render_state = cc.wgpu_render_state.as_ref().unwrap();
        let paint_callback_resources = &mut render_state.renderer.write().paint_callback_resources;

        // TODO(andreas): Query used surface format from eframe/renderer.
        #[cfg(target_arch = "wasm32")]
        let (output_format_color, output_format_depth) =
            (wgpu::TextureFormat::Rgba8UnormSrgb, None); // TODO(andreas) fix for not using srgb is in flight!
        #[cfg(not(target_arch = "wasm32"))]
        let (output_format_color, output_format_depth) = (
            wgpu::TextureFormat::Bgra8Unorm,
            Some(wgpu::TextureFormat::Depth32Float),
        );

        paint_callback_resources.insert(RenderContext::new(
            &render_state.device,
            &render_state.queue,
            output_format_color,
            output_format_depth,
        ));
    }

    design_tokens::apply_design_tokens(&cc.egui_ctx);
}
