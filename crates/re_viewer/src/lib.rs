//! Rerun Viewer GUI.
//!
//! This crate contains all the GUI code for the Rerun Viewer,
//! including all 2D and 3D visualization code.

mod app;
pub mod env_vars;
pub mod math;
mod misc;
mod remote_viewer_app;
mod ui;
mod viewer_analytics;

pub(crate) use misc::{mesh_loader, Item, TimeControl, TimeView, ViewerContext};
use re_log_types::PythonVersion;
pub(crate) use ui::{event_log_view, memory_panel, selection_panel, time_panel, UiVerbosity};

pub use app::{App, StartupOptions};
pub use remote_viewer_app::RemoteViewerApp;

pub mod external {
    pub use eframe;
    pub use egui;
    pub use re_renderer;
}

// ----------------------------------------------------------------------------
// When compiling for native:

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{run_native_app, run_native_viewer_with_messages};

mod app_icon;

#[cfg(not(target_arch = "wasm32"))]
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

/// Where is this App running in?
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

#[allow(dead_code)]
const APPLICATION_NAME: &str = "Rerun Viewer";

pub(crate) fn hardware_tier() -> re_renderer::config::HardwareTier {
    re_renderer::config::HardwareTier::default()
}

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
            // TODO(andreas): This should be the default for egui-wgpu.
            power_preference: wgpu::util::power_preference_from_env().unwrap_or(wgpu::PowerPreference::HighPerformance),
            ..Default::default()
        }
}

#[must_use]
pub(crate) fn customize_eframe(cc: &eframe::CreationContext<'_>) -> re_ui::ReUi {
    if let Some(render_state) = &cc.wgpu_render_state {
        use re_renderer::{config::RenderContextConfig, RenderContext};

        let paint_callback_resources = &mut render_state.renderer.write().paint_callback_resources;

        paint_callback_resources.insert(RenderContext::new(
            render_state.device.clone(),
            render_state.queue.clone(),
            RenderContextConfig {
                output_format_color: render_state.target_format,
                hardware_tier: crate::hardware_tier(),
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
            while let Ok((sent_at, msg)) = rx.recv_with_send_time() {
                if tx.send_at(sent_at, msg).is_ok() {
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

// ---------------------------------------------------------------------------

pub fn stream_rrd_from_http_to_channel(
    url: String,
) -> re_smart_channel::Receiver<re_log_types::LogMsg> {
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::RrdHttpStream {
        url: url.clone(),
    });
    stream_rrd_from_http(
        url,
        Box::new(move |msg| {
            tx.send(msg).ok();
        }),
    );
    rx
}

pub fn stream_rrd_from_http(url: String, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
    re_log::debug!("Downloading .rrd file from {url:?}…");

    // TODO(emilk): stream the http request, progressively decoding the .rrd file.
    ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
        Ok(response) => {
            if response.ok {
                re_log::debug!("Decoding .rrd file from {url:?}…");
                decode_rrd(response.bytes, on_msg);
            } else {
                re_log::error!(
                    "Failed to fetch .rrd file from {url}: {} {}",
                    response.status,
                    response.status_text
                );
            }
        }
        Err(err) => {
            re_log::error!("Failed to fetch .rrd file from {url}: {err}");
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::needless_pass_by_value)] // must match wasm version
fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
    match re_log_types::encoding::Decoder::new(rrd_bytes.as_slice()) {
        Ok(decoder) => {
            for msg in decoder {
                match msg {
                    Ok(msg) => {
                        on_msg(msg);
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to decode message: {err}");
                    }
                }
            }
        }
        Err(err) => {
            re_log::error!("Failed to decode .rrd: {err}");
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web_decode {
    pub fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
        wasm_bindgen_futures::spawn_local(decode_rrd_async(rrd_bytes, on_msg))
    }

    /// Decodes the file in chunks, with an yield between each chunk.
    ///
    /// This is cooperative multi-tasking.
    async fn decode_rrd_async(
        rrd_bytes: Vec<u8>,
        on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>,
    ) {
        let mut last_yield = instant::Instant::now();

        match re_log_types::encoding::Decoder::new(rrd_bytes.as_slice()) {
            Ok(decoder) => {
                for msg in decoder {
                    match msg {
                        Ok(msg) => {
                            on_msg(msg);
                        }
                        Err(err) => {
                            re_log::warn_once!("Failed to decode message: {err}");
                        }
                    }

                    if last_yield.elapsed() > instant::Duration::from_millis(10) {
                        // yield to the ui task
                        yield_().await;
                        last_yield = instant::Instant::now();
                    }
                }
            }
            Err(err) => {
                re_log::error!("Failed to decode .rrd: {err}");
            }
        }
    }

    // Yield to other tasks
    async fn yield_() {
        sleep_ms(1).await; // TODO(emilk): create a better async yield function
    }

    // Hack to get async sleep on wasm
    async fn sleep_ms(millis: i32) {
        let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
                .expect("Failed to call set_timeout");
        };
        let p = js_sys::Promise::new(&mut cb);
        wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
    }
}

#[cfg(target_arch = "wasm32")]
use web_decode::decode_rrd;
