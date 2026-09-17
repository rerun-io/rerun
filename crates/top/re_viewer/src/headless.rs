//! Headless viewer driven by [`egui_kittest`] instead of a real eframe window.
//!
//! Used for things like CI screenshot generation via `ViewerClient::save_screenshot`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

use crate::App;

type AppCreator = Box<dyn FnOnce(&eframe::CreationContext<'_>) -> App>;

/// Earliest time at which egui has asked us to repaint, if any.
type RepaintSignal = (Mutex<Option<Instant>>, Condvar);

/// Default headless viewport size (logical points).
const DEFAULT_HEADLESS_SIZE: (f32, f32) = (1920.0, 1080.0);

/// How long an idle headless viewer sleeps between frames.
const IDLE_TICK: Duration = Duration::from_secs(1);

/// Shortest time between two headless frames, i.e. a 60 FPS cap.
///
/// There is no vsync to pace us, so without this the loop renders as fast as
/// the machine allows whenever anything asks for a repaint every frame (video
/// decoding, animations, a spinner, …).
const MIN_FRAME_TIME: Duration = Duration::from_millis(16);

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

    let repaint_signal: Arc<RepaintSignal> = Arc::new((Mutex::new(None), Condvar::new()));

    let mut init_result = Ok(());
    let init_result_mut = &mut init_result;

    let mut harness = {
        let repaint_signal = repaint_signal.clone();
        egui_kittest::Harness::<App>::builder()
            .with_size(size)
            .wgpu_setup(wgpu_setup)
            .build_eframe(move |cc| {
                let repaint_signal = repaint_signal.clone();
                cc.egui_ctx.set_request_repaint_callback(move |info| {
                    let deadline = Instant::now() + info.delay;
                    let (lock, cvar) = &*repaint_signal;
                    let mut earliest = lock.lock();
                    if earliest.is_none_or(|current| deadline < current) {
                        *earliest = Some(deadline);
                        cvar.notify_all();
                    }
                });
                *init_result_mut = crate::customize_eframe_and_setup_renderer(cc);
                app_creator(cc)
            })
    };

    init_result.map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;

    re_log::info!("Headless viewer running at {}x{}.", size.x, size.y);

    loop {
        let frame_start = Instant::now();

        harness.step();

        if has_pending_close(&harness) {
            re_log::info!("Headless viewer received close request, shutting down.");
            return Ok(());
        }

        wait_for_repaint(&repaint_signal, frame_start + MIN_FRAME_TIME);
    }
}

/// Block until a repaint is due, or until the idle tick elapses.
///
/// Never returns before `frame_floor`, which paces the loop even when every
/// frame asks for an immediate repaint.
///
/// `request_repaint_after(delay)` must not wake us before `delay` has passed:
/// treating a delayed request as "repaint now" turns this loop into a busy spin
/// that burns a full core, since headless rendering has no vsync to throttle it.
fn wait_for_repaint(repaint_signal: &RepaintSignal, frame_floor: Instant) {
    let (lock, cvar) = repaint_signal;
    let idle_deadline = Instant::now() + IDLE_TICK;

    let mut requested = lock.lock();
    loop {
        let deadline = requested
            .map_or(idle_deadline, |deadline| deadline.min(idle_deadline))
            .max(frame_floor);
        let now = Instant::now();
        if deadline <= now {
            break;
        }
        cvar.wait_for(&mut requested, deadline - now);
    }
    *requested = None;
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
