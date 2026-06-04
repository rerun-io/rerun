//! Headless viewer driven by [`egui_kittest`] instead of a real eframe window.
//!
//! Used for things like CI screenshot generation via `ViewerClient::save_screenshot`.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Condvar, Mutex};

use crate::App;

type AppCreator = Box<dyn FnOnce(&eframe::CreationContext<'_>) -> App>;

/// Default headless viewport size (logical points).
const DEFAULT_HEADLESS_SIZE: (f32, f32) = (1920.0, 1080.0);

/// Run the viewer in headless mode.
///
/// Instead of opening a real OS window via `eframe::run_native`, this drives the
/// viewer through an `egui_kittest` harness backed by `wgpu`, repeatedly calling
/// `step()`. The gRPC server keeps running in the background just like in the
/// normal viewer, so SDK clients (including `save_screenshot`) work the same way.
///
/// Blocks until the process is killed.
pub fn run_headless_app(
    app_creator: AppCreator,
    force_wgpu_backend: Option<&str>,
    initial_size: Option<egui::Vec2>,
) -> eframe::Result {
    let size = initial_size
        .unwrap_or_else(|| egui::vec2(DEFAULT_HEADLESS_SIZE.0, DEFAULT_HEADLESS_SIZE.1));

    let wgpu_setup = crate::wgpu_options(force_wgpu_backend).wgpu_setup;

    // Signal flipped to `true` whenever something calls `ctx.request_repaint()`.
    // The headless loop uses this to wake up early instead of waiting the full
    // 1s idle tick — keeps animations and incoming gRPC data feeling snappy
    // while still letting an idle viewer sleep most of the time.
    let repaint_signal: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));

    let mut init_result = Ok(());
    let init_result_mut = &mut init_result;

    let mut harness = {
        let repaint_signal = repaint_signal.clone();
        egui_kittest::Harness::<App>::builder()
            .with_size(size)
            .wgpu_setup(wgpu_setup)
            .build_eframe(move |cc| {
                let repaint_signal = repaint_signal.clone();
                cc.egui_ctx.set_request_repaint_callback(move |_info| {
                    let (lock, cvar) = &*repaint_signal;
                    *lock.lock() = true;
                    cvar.notify_all();
                });
                *init_result_mut = crate::customize_eframe_and_setup_renderer(cc);
                app_creator(cc)
            })
    };

    init_result.map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;

    re_log::info!("Headless viewer running at {}x{}.", size.x, size.y);

    let idle_timeout = Duration::from_secs(1);
    loop {
        harness.step();
        handle_pending_screenshots(&mut harness);

        if has_pending_close(&harness) {
            re_log::info!("Headless viewer received close request, shutting down.");
            return Ok(());
        }

        let (lock, cvar) = &*repaint_signal;
        let mut signaled = lock.lock();
        if !*signaled {
            cvar.wait_for(&mut signaled, idle_timeout);
        }
        *signaled = false;
    }
}

/// Detect `ViewportCommand::Close` in this frame's viewport output.
///
/// `UICommand::Quit` (and the Ctrl-C handler) ultimately send
/// `ViewportCommand::Close`. In a normal `eframe::run_native` setup the
/// windowing backend consumes that and exits the event loop. `kittest`
/// ignores viewport commands, so we have to detect `Close` here and break
/// out of the headless loop ourselves.
fn has_pending_close(harness: &egui_kittest::Harness<'_, App>) -> bool {
    harness
        .output()
        .viewport_output
        .values()
        .flat_map(|v| v.commands.iter())
        .any(|cmd| matches!(cmd, egui::ViewportCommand::Close))
}

/// Bridge [`egui::ViewportCommand::Screenshot`] requests through `kittest`'s
/// offscreen renderer.
///
/// In a normal `eframe::run_native` setup, the windowing backend captures the
/// framebuffer after a screenshot command and emits an
/// [`egui::Event::Screenshot`] that the viewer's `App` listens for. `kittest`
/// doesn't process viewport commands itself, so we have to do that translation
/// here, otherwise `save_screenshot` requests would be silently dropped.
fn handle_pending_screenshots(harness: &mut egui_kittest::Harness<'_, App>) {
    let pending: Vec<egui::UserData> = harness
        .output()
        .viewport_output
        .values()
        .flat_map(|v| v.commands.iter())
        .filter_map(|cmd| match cmd {
            egui::ViewportCommand::Screenshot(user_data) => Some(user_data.clone()),
            _ => None,
        })
        .collect();

    if pending.is_empty() {
        return;
    }

    let rgba = match harness.render() {
        Ok(rgba) => rgba,
        Err(err) => {
            re_log::error!("Failed to render headless screenshot: {err}");
            return;
        }
    };
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    let color_image = Arc::new(egui::ColorImage::from_rgba_premultiplied(size, &pixels));

    for user_data in pending {
        harness.event(egui::Event::Screenshot {
            viewport_id: egui::ViewportId::ROOT,
            user_data,
            image: color_image.clone(),
        });
    }
}
