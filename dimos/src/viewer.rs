//! DimOS Interactive Viewer — custom Rerun viewer with WebSocket click-to-navigate and WASD teleop.
//!
//! Accepts ALL stock Rerun CLI flags and adds DimOS-specific behavior:
//! - Click-to-navigate: click any entity with a 3D position → sends click event via WebSocket
//! - WASD keyboard teleop: click overlay to engage, then WASD publishes Twist via WebSocket

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dimos_viewer::interaction::{KeyboardHandler, WsCommand, WsPublisher};
use rerun::external::{eframe, egui, re_log, re_memory, re_viewer};

#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

/// Default WebSocket server URL
const DEFAULT_WS_URL: &str = "ws://127.0.0.1:3030/ws";

/// Minimum time between click events (debouncing)
const CLICK_DEBOUNCE_MS: u64 = 100;

/// Maximum rapid clicks before logging a warning
const RAPID_CLICK_THRESHOLD: usize = 5;

/// Wraps re_viewer::App to add keyboard teleop overlay.
struct DimosApp {
    inner: re_viewer::App,
    keyboard: KeyboardHandler,
    ws_publisher: WsPublisher,
}

impl DimosApp {
    fn handle_ws_commands(&mut self) {
        while let Some(command) = self.ws_publisher.try_recv_command() {
            if let Err(err) = command.validate() {
                re_log::warn!("Ignoring invalid websocket command: {err}");
                continue;
            }

            match command {
                WsCommand::OpenWebPageView {
                    panel_id,
                    title,
                    url,
                    show_navigation_controls,
                } => {
                    self.inner
                        .open_or_update_web_page_view(re_viewer::WebPageViewRequest {
                            panel_id,
                            title,
                            url,
                            show_navigation_controls,
                        });
                }
            }
        }
    }
}

impl eframe::App for DimosApp {
    /// Called before `ui` every frame (and on hidden repaints).
    /// re_viewer::App drains log_receivers / ingests messages here, so we MUST
    /// forward — otherwise the viewer's data pipeline stalls.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.inner.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.keyboard.process(ui.ctx());
        let keyboard_overlay_rect = self.keyboard.draw_overlay(ui.ctx());
        self.inner
            .set_web_page_overlay_clip_rect(keyboard_overlay_rect);
        self.handle_ws_commands();
        self.inner.ui(ui, frame);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.inner.save(storage);
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.inner.clear_color(visuals)
    }

    fn persist_egui_memory(&self) -> bool {
        self.inner.persist_egui_memory()
    }

    fn auto_save_interval(&self) -> Duration {
        self.inner.auto_save_interval()
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.inner.raw_input_hook(ctx, raw_input);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread_token = re_viewer::MainThreadToken::i_promise_i_am_on_the_main_thread();
    re_log::setup_logging();
    let build_info = re_viewer::build_info();

    // Parse args (including --ws-url) via Rerun's clap Args, without consuming them.
    // We peek at the parsed value, then pass the original args to run_with_app_wrapper
    // which will parse them again.
    let parsed: rerun::RerunArgs = clap::Parser::parse();
    let ws_url = if std::env::var("DIMOS_VIEWER_WS_URL").is_ok() {
        // Env var overrides default but not an explicit CLI flag.
        // If the parsed value equals the default, check the env var.
        if parsed.ws_url == DEFAULT_WS_URL {
            std::env::var("DIMOS_VIEWER_WS_URL").unwrap()
        } else {
            parsed.ws_url.clone()
        }
    } else {
        parsed.ws_url.clone()
    };

    let debug = std::env::var("DIMOS_DEBUG").is_ok_and(|v| v == "1");

    // Connect WebSocket publisher for click/keyboard events
    let ws_publisher = WsPublisher::connect(ws_url.clone());
    if debug {
        eprintln!("[DIMOS_DEBUG] WebSocket client target: {ws_url}");
    }

    let keyboard_handler_ws = ws_publisher.clone();
    let command_ws = ws_publisher.clone();

    let last_click_time = Rc::new(RefCell::new(Instant::now() - Duration::from_secs(10)));
    let rapid_click_count = Rc::new(RefCell::new(0usize));

    // Plain click (no Ctrl required) fires nav goal on any entity with a 3D position
    let startup_patch = rerun::StartupOptionsPatch {
        on_event: Some(Rc::new(move |event: re_viewer::ViewerEvent| {
            if let re_viewer::ViewerEventKind::SelectionChange { items } = event.kind {
                let mut has_position = false;
                let mut no_position_count = 0;

                for item in &items {
                    match item {
                        re_viewer::SelectionChangeItem::Entity {
                            entity_path,
                            position: Some(pos),
                            ..
                        } => {
                            has_position = true;

                            let now = Instant::now();
                            let elapsed = now.duration_since(*last_click_time.borrow());

                            if elapsed < Duration::from_millis(CLICK_DEBOUNCE_MS) {
                                let mut count = rapid_click_count.borrow_mut();
                                *count += 1;
                                if *count == RAPID_CLICK_THRESHOLD {
                                    re_log::warn!(
                                        "Rapid click detected ({RAPID_CLICK_THRESHOLD} clicks within {CLICK_DEBOUNCE_MS}ms)"
                                    );
                                }
                                continue;
                            } else {
                                *rapid_click_count.borrow_mut() = 0;
                            }
                            *last_click_time.borrow_mut() = now;

                            let timestamp_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;

                            if let Err(err) = ws_publisher.send_click(
                                pos.x as f64,
                                pos.y as f64,
                                pos.z as f64,
                                &entity_path.to_string(),
                                timestamp_ms,
                            ) {
                                re_log::warn!("Failed to send click event: {err}");
                            }
                            re_log::debug!(
                                "Click event published: entity={}, pos=({:.2}, {:.2}, {:.2})",
                                entity_path,
                                pos.x,
                                pos.y,
                                pos.z
                            );
                        }
                        re_viewer::SelectionChangeItem::Entity { position: None, .. } => {
                            no_position_count += 1;
                        }
                        _ => {}
                    }
                }

                if !has_position && no_position_count > 0 {
                    re_log::trace!(
                        "Selection change without position ({no_position_count} items) — normal for hover/keyboard nav."
                    );
                }
            }
        })),
    };

    if debug {
        if let Some(ref connect) = parsed.connect {
            match connect.as_deref() {
                Some(url) => eprintln!("[DIMOS_DEBUG] gRPC connecting to: {url}"),
                None => eprintln!(
                    "[DIMOS_DEBUG] gRPC connecting to default (port {})",
                    parsed.port
                ),
            }
        } else {
            eprintln!(
                "[DIMOS_DEBUG] gRPC: starting local server on port {}",
                parsed.port
            );
        }
    }

    let wrapper: rerun::AppWrapper = Box::new(move |app| {
        let keyboard = KeyboardHandler::new(keyboard_handler_ws.clone());
        Ok(Box::new(DimosApp {
            inner: app,
            keyboard,
            ws_publisher: command_ws.clone(),
        }))
    });

    let exit_code = rerun::run_with_app_wrapper(
        main_thread_token,
        build_info,
        rerun::CallSource::Cli,
        std::env::args(),
        Some(wrapper),
        Some(startup_patch),
    )?;

    std::process::exit(exit_code.into());
}
