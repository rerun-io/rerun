//! Rerun Viewer GUI.
//!
//! This crate contains all the GUI code for the Rerun Viewer,
//! including all 2D and 3D visualization code.

mod app;
pub mod blueprint_components;
pub mod env_vars;
#[cfg(not(target_arch = "wasm32"))]
mod profiler;
mod remote_viewer_app;
mod ui;
mod viewer_analytics;

use re_log_types::PythonVersion;
pub(crate) use ui::{memory_panel, selection_panel};

pub use app::{App, StartupOptions};
pub use remote_viewer_app::RemoteViewerApp;

pub mod external {
    pub use {eframe, egui};
    pub use {
        re_arrow_store, re_arrow_store::external::arrow2, re_data_store, re_log, re_log_types,
        re_memory, re_renderer, re_ui, re_viewer_context, re_viewer_context::external::re_query,
        re_viewport,
    };
}

// ----------------------------------------------------------------------------
// When compiling for native:

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{run_native_app, run_native_viewer_with_messages};

#[cfg(not(target_arch = "wasm32"))]
pub use profiler::Profiler;

// ----------------------------------------------------------------------------
// When compiling for web:

#[cfg(target_arch = "wasm32")]
mod web;

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}

// ---------------------------------------------------------------------------

/// Information about this version of the crate.
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}

// ---------------------------------------------------------------------------

/// Where is this App running in?
/// Used for analytics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppEnvironment {
    /// Created from the Rerun Python SDK.
    PythonSdk(PythonVersion),

    /// Created from the Rerun Rust SDK.
    RustSdk {
        rustc_version: String,
        llvm_version: String,
    },

    /// Running the Rust `rerun` binary from the CLI.
    RerunCli {
        rustc_version: String,
        llvm_version: String,
    },

    /// We are a web-viewer running in a browser as Wasm.
    Web,

    /// Some custom application wrapping re_viewer
    Custom(String),
}

impl AppEnvironment {
    pub fn from_recording_source(source: &re_log_types::RecordingSource) -> Self {
        use re_log_types::RecordingSource;
        match source {
            RecordingSource::PythonSdk(python_version) => Self::PythonSdk(python_version.clone()),
            RecordingSource::RustSdk {
                rustc_version: rust_version,
                llvm_version,
            } => Self::RustSdk {
                rustc_version: rust_version.clone(),
                llvm_version: llvm_version.clone(),
            },
            RecordingSource::Unknown | RecordingSource::Other(_) => Self::RustSdk {
                rustc_version: "unknown".into(),
                llvm_version: "unknown".into(),
            },
        }
    }
}

// ---------------------------------------------------------------------------

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
            supported_backends: re_renderer::config::supported_backends(),
            device_descriptor: std::sync::Arc::new(|adapter| re_renderer::config::HardwareTier::from_adapter(adapter).device_descriptor()),
            ..Default::default()
        }
}

/// Customize eframe and egui to suit the rerun viewer.
#[must_use]
pub fn customize_eframe(cc: &eframe::CreationContext<'_>) -> re_ui::ReUi {
    if let Some(render_state) = &cc.wgpu_render_state {
        use re_renderer::{config::RenderContextConfig, RenderContext};

        let paint_callback_resources = &mut render_state.renderer.write().paint_callback_resources;

        paint_callback_resources.insert(RenderContext::new(
            &render_state.adapter,
            render_state.device.clone(),
            render_state.queue.clone(),
            RenderContextConfig {
                output_format_color: render_state.target_format,
                hardware_tier: re_renderer::config::HardwareTier::from_adapter(
                    &render_state.adapter,
                ),
            },
        ));
    }

    re_ui::ReUi::load_and_apply(&cc.egui_ctx)
}

// ---------------------------------------------------------------------------

/// This wakes up the ui thread each time we receive a new message.
#[cfg(not(feature = "web_viewer"))]
#[cfg(not(target_arch = "wasm32"))]
pub fn wake_up_ui_thread_on_each_msg<T: Send + 'static>(
    rx: re_smart_channel::Receiver<T>,
    ctx: egui::Context,
) -> re_smart_channel::Receiver<T> {
    // We need to intercept messages to wake up the ui thread.
    // For that, we need a new channel.
    // However, we want to make sure the channel latency numbers are from the start
    // of the first channel, to the end of the second.
    // For that we need to use `chained_channel`, `recv_with_send_time` and `send_at`.
    let (tx, new_rx) = rx.chained_channel();
    std::thread::Builder::new()
        .name("ui_waker".to_owned())
        .spawn(move || {
            while let Ok(msg) = rx.recv_with_send_time() {
                if tx.send_at(msg.time, msg.source, msg.payload).is_ok() {
                    ctx.request_repaint();
                } else {
                    break;
                }
            }
            re_log::debug!("Shutting down ui_waker thread");
        })
        .unwrap();
    new_rx
}
