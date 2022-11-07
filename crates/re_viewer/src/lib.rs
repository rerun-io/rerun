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

pub(crate) use design_tokens::DesignTokens;
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

/// If true, we fill the entire window, except for the close/maximize/minimize buttons in the top-left.
/// See <https://github.com/emilk/egui/pull/2049>
pub const FULLSIZE_CONTENT: bool = cfg!(target_os = "macos");

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

#[cfg(feature = "wgpu")]
pub(crate) fn hardware_tier() -> re_renderer::config::HardwareTier {
    re_renderer::config::HardwareTier::Web
}

#[cfg(feature = "wgpu")]
pub(crate) fn wgpu_options() -> egui_wgpu::WgpuConfiguration {
    egui_wgpu::WgpuConfiguration {
            // When running wgpu on native debug builds, we want some extra control over how
            // and when a poisoned surface gets recreated.
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // native debug build
            on_surface_error: std::sync::Arc::new(|err| {
                // On windows, this error also occurs when the app is minimized.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain
                //  must be updated"
                if err == wgpu::SurfaceError::Outdated && !cfg!(target_os = "windows"){
                    // We haven't been able to present anything to the swapchain for
                    // a while, because the pipeline is poisoned.
                    // Recreate a sane surface to restart the cycle and see if the
                    // user has fixed the issue.
                    egui_wgpu::SurfaceErrorAction::RecreateSurface
                } else {
                    egui_wgpu::SurfaceErrorAction::SkipFrame
                }
            }),
            backends: re_renderer::config::supported_backends(),
            device_descriptor: crate::hardware_tier().device_descriptor(),
            ..Default::default()
        }
}

pub(crate) fn customize_eframe(cc: &eframe::CreationContext<'_>) -> crate::DesignTokens {
    #[cfg(feature = "wgpu")]
    if let Some(render_state) = &cc.wgpu_render_state {
        use re_renderer::{config::RenderContextConfig, RenderContext};

        let paint_callback_resources = &mut render_state.renderer.write().paint_callback_resources;

        // TODO(andreas): Query used surface format from eframe/renderer.
        let output_format_color = if cfg!(target_arch = "wasm32") {
            wgpu::TextureFormat::Rgba8Unorm
        } else {
            wgpu::TextureFormat::Bgra8Unorm
        };

        paint_callback_resources.insert(RenderContext::new(
            &render_state.device,
            &render_state.queue,
            RenderContextConfig {
                output_format_color,
                hardware_tier: crate::hardware_tier(),
            },
        ));
    }

    design_tokens::apply_design_tokens(&cc.egui_ctx)
}
