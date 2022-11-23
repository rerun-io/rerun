use std::{any::Any, sync::mpsc::Receiver};

use ahash::HashMap;
use egui_extras::RetainedImage;
use egui_notify::Toasts;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use poll_promise::Promise;

use re_data_store::log_db::LogDb;
use re_log_types::*;

use crate::{
    design_tokens::DesignTokens,
    misc::{Caches, Options, RecordingConfig, ViewerContext},
    ui::kb_shortcuts,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::misc::TimeRangeF;

const WATERMARK: bool = false; // Nice for recording media material

// ----------------------------------------------------------------------------

/// The Rerun viewer as an [`eframe`] application.
pub struct App {
    design_tokens: DesignTokens,

    rx: Option<Receiver<LogMsg>>,

    /// Where the logs are stored.
    log_dbs: IntMap<RecordingId, LogDb>,

    /// What is serialized
    state: AppState,

    /// Set to `true` on Ctrl-C.
    #[cfg(not(target_arch = "wasm32"))]
    ctrl_c: std::sync::Arc<std::sync::atomic::AtomicBool>,

    /// Pending background tasks, using `poll_promise`.
    pending_promises: HashMap<String, Promise<Box<dyn Any + Send>>>,

    /// Toast notifications, using `egui-notify`.
    toasts: Toasts,

    latest_memory_purge: instant::Instant,
    memory_panel: crate::memory_panel::MemoryPanel,
    memory_panel_open: bool,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        egui_ctx: &egui::Context,
        design_tokens: DesignTokens,
        storage: Option<&dyn eframe::Storage>,
        rx: Receiver<LogMsg>,
    ) -> Self {
        Self::new(
            egui_ctx,
            design_tokens,
            storage,
            Some(rx),
            Default::default(),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_log_db(
        egui_ctx: &egui::Context,
        design_tokens: DesignTokens,
        storage: Option<&dyn eframe::Storage>,
        log_db: LogDb,
    ) -> Self {
        Self::new(egui_ctx, design_tokens, storage, None, log_db)
    }

    /// load a `.rrd` data file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_rrd_path(
        egui_ctx: &egui::Context,
        design_tokens: DesignTokens,
        storage: Option<&dyn eframe::Storage>,
        path: &std::path::Path,
    ) -> Self {
        let log_db = load_file_path(path).unwrap_or_default(); // TODO(emilk): exit on error.
        Self::from_log_db(egui_ctx, design_tokens, storage, log_db)
    }

    fn new(
        _egui_ctx: &egui::Context,
        design_tokens: DesignTokens,
        storage: Option<&dyn eframe::Storage>,
        rx: Option<Receiver<LogMsg>>,
        log_db: LogDb,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let ctrl_c = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Close viewer on Ctrl-C. TODO(emilk): maybe add to `eframe`?

            let ctrl_c = ctrl_c.clone();
            let egui_ctx = _egui_ctx.clone();

            ctrlc::set_handler(move || {
                re_log::debug!("Ctrl-C detected - Closing viewer.");
                ctrl_c.store(true, std::sync::atomic::Ordering::SeqCst);
                egui_ctx.request_repaint(); // so that we notice that we should close
            })
            .expect("Error setting Ctrl-C handler");
        }

        let mut state: AppState = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        let mut log_dbs = IntMap::default();
        if !log_db.is_empty() {
            state.selected_rec_id = log_db.recording_id();
            log_dbs.insert(log_db.recording_id(), log_db);
        }

        Self {
            design_tokens,
            rx,
            log_dbs,
            state,
            #[cfg(not(target_arch = "wasm32"))]
            ctrl_c,
            pending_promises: Default::default(),
            toasts: Toasts::new(),
            latest_memory_purge: instant::Instant::now() - std::time::Duration::from_secs(10_000),
            memory_panel: Default::default(),
            memory_panel_open: false,
        }
    }

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        self.state.profiler = profiler;
    }

    /// Creates a promise with the specified name that will run `f` on a background
    /// thread using the `poll_promise` crate.
    ///
    /// Names can only be re-used once the promise with that name has finished running,
    /// otherwise an other is returned.
    // TODO(cmc): offer `spawn_async_promise` once we open save_file to the web
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_threaded_promise<F, T>(
        &mut self,
        name: impl Into<String>,
        f: F,
    ) -> anyhow::Result<()>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let name = name.into();

        if self.pending_promises.contains_key(&name) {
            anyhow::bail!("there's already a promise {name:?} running!");
        }

        let f = move || Box::new(f()) as Box<dyn Any + Send>; // erase it
        let promise = Promise::spawn_thread(&name, f);

        self.pending_promises.insert(name, promise);

        Ok(())
    }

    /// Polls the promise with the given name.
    ///
    /// Returns `Some<T>` it it's ready, or `None` otherwise.
    ///
    /// Panics if `T` does not match the actual return value of the promise.
    pub fn poll_promise<T: Any>(&mut self, name: impl AsRef<str>) -> Option<T> {
        self.pending_promises
            .remove(name.as_ref())
            .and_then(|promise| match promise.try_take() {
                Ok(any) => Some(*any.downcast::<T>().unwrap()),
                Err(promise) => {
                    self.pending_promises
                        .insert(name.as_ref().to_owned(), promise);
                    None
                }
            })
    }

    /// Returns whether a promise with the given name is currently running.
    pub fn promise_exists(&mut self, name: impl AsRef<str>) -> bool {
        self.pending_promises.contains_key(name.as_ref())
    }

    fn check_keyboard_shortcuts(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if egui_ctx
            .input_mut()
            .consume_shortcut(&kb_shortcuts::RESET_VIEWER)
        {
            self.reset(egui_ctx);
        }

        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        if egui_ctx
            .input_mut()
            .consume_shortcut(&kb_shortcuts::SHOW_PROFILER)
        {
            self.state.profiler.start();
        }

        if egui_ctx
            .input_mut()
            .consume_shortcut(&kb_shortcuts::TOGGLE_MEMORY_PANEL)
        {
            self.memory_panel_open ^= true;
        }

        if !frame.is_web() {
            egui::gui_zoom::zoom_with_keyboard_shortcuts(
                egui_ctx,
                frame.info().native_pixels_per_point,
            );
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.memory_panel.update(); // do first, before doing too many allocations

        #[cfg(not(target_arch = "wasm32"))]
        if self.ctrl_c.load(std::sync::atomic::Ordering::Relaxed) {
            frame.close();
            return;
        }

        self.check_keyboard_shortcuts(egui_ctx, frame);

        self.purge_memory_if_needed();

        self.state.cache.new_frame();

        self.receive_messages(egui_ctx);

        self.cleanup();

        file_saver_progress_ui(egui_ctx, self); // toasts for background file saver
        top_panel(egui_ctx, frame, self);

        egui::TopBottomPanel::bottom("memory_panel")
            .default_height(300.0)
            .resizable(true)
            .show_animated(egui_ctx, self.memory_panel_open, |ui| {
                self.memory_panel.ui(ui);
            });

        let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();

        self.state
            .recording_configs
            .entry(self.state.selected_rec_id)
            .or_default()
            .on_frame_start();

        // TODO(andreas): store the re_renderer somewhere else.
        let egui_renderer = {
            let render_state = frame.wgpu_render_state().unwrap();
            &mut render_state.renderer.write()
        };
        let render_ctx = egui_renderer
            .paint_callback_resources
            .get_mut::<re_renderer::RenderContext>()
            .unwrap();
        render_ctx.frame_maintenance();

        if log_db.is_empty() && self.rx.is_some() {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.heading("Waiting for data…"); // TODO(emilk): show what ip/port we are listening to
                });
            });
        } else {
            self.state
                .show(egui_ctx, log_db, &self.design_tokens, render_ctx);
        }

        self.handle_dropping_files(egui_ctx);
        self.toasts.show(egui_ctx);
    }
}

impl App {
    fn receive_messages(&mut self, egui_ctx: &egui::Context) {
        if let Some(rx) = &mut self.rx {
            crate::profile_function!();
            let start = instant::Instant::now();

            while let Ok(msg) = rx.try_recv() {
                if let LogMsg::BeginRecordingMsg(msg) = &msg {
                    re_log::info!("Beginning a new recording: {:?}", msg.info);
                    self.state.selected_rec_id = msg.info.recording_id;
                }

                let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();

                log_db.add(msg);
                if start.elapsed() > instant::Duration::from_millis(10) {
                    egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                    break; // don't block the main thread for too long
                }
            }
        }
    }

    fn cleanup(&mut self) {
        crate::profile_function!();

        self.log_dbs.retain(|_, log_db| !log_db.is_empty());

        if !self.log_dbs.contains_key(&self.state.selected_rec_id) {
            self.state.selected_rec_id = self.log_dbs.keys().next().cloned().unwrap_or_default();
        }

        self.state
            .recording_configs
            .retain(|recording_id, _| self.log_dbs.contains_key(recording_id));

        if self.state.blueprints.len() > 100 {
            re_log::debug!("Pruning blueprints…");

            let used_app_ids: std::collections::HashSet<ApplicationId> = self
                .log_dbs
                .values()
                .filter_map(|log_db| {
                    log_db
                        .recording_info()
                        .map(|recording_info| recording_info.application_id.clone())
                })
                .collect();

            self.state
                .blueprints
                .retain(|application_id, _| used_app_ids.contains(application_id));
        }
    }

    fn purge_memory_if_needed(&mut self) {
        crate::profile_function!();

        fn format_limit(limit: Option<i64>) -> String {
            if let Some(bytes) = limit {
                format_bytes(bytes as _)
            } else {
                "∞".to_owned()
            }
        }

        use re_memory::{util::format_bytes, MemoryUse};

        if self.latest_memory_purge.elapsed() < instant::Duration::from_secs(10) {
            // Pruning introduces stutter, and we don't want to stutter too often.
            return;
        }

        let limit = re_memory::MemoryLimit::from_env_var(crate::env_vars::RERUN_MEMORY_LIMIT);
        let mem_use_before = MemoryUse::capture();

        if let Some(minimum_fraction_to_free) = limit.is_exceeded_by(&mem_use_before) {
            let fraction_to_free = (minimum_fraction_to_free + 0.2).clamp(0.25, 1.0);

            re_log::debug!("RAM limit: {}", format_limit(limit.limit));
            if let Some(resident) = mem_use_before.resident {
                re_log::debug!("Using {} according to OS", format_bytes(resident as _),);
            }
            if let Some(net) = mem_use_before.net {
                re_log::debug!("Actually used: {}", format_bytes(net as _));
            }

            {
                crate::profile_scope!("pruning");
                re_log::info!(
                    "Attempting to purge {:.1}% of used RAM…",
                    100.0 * fraction_to_free
                );
                for log_db in self.log_dbs.values_mut() {
                    log_db.purge_memory(fraction_to_free);
                }
                self.state.cache.purge_memory();
            }

            let mem_use_after = MemoryUse::capture();

            let freed_memory = mem_use_before - mem_use_after;

            if let Some(net_diff) = freed_memory.net {
                re_log::info!("Freed up {}", format_bytes(net_diff as _));
            }

            self.latest_memory_purge = instant::Instant::now();

            self.memory_panel.note_memory_purge();
        }
    }

    /// Reset the viewer to how it looked the first time you ran it.
    fn reset(&mut self, egui_ctx: &egui::Context) {
        let selected_rec_id = self.state.selected_rec_id;

        self.state = Default::default();
        self.state.selected_rec_id = selected_rec_id;

        // Keep dark/light mode setting:
        let is_dark_mode = egui_ctx.style().visuals.dark_mode;
        *egui_ctx.memory() = Default::default();
        egui_ctx.set_visuals(if is_dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }

    fn log_db(&mut self) -> &mut LogDb {
        self.log_dbs.entry(self.state.selected_rec_id).or_default()
    }

    fn show_log_db(&mut self, log_db: LogDb) {
        self.state.selected_rec_id = log_db.recording_id();
        self.log_dbs.insert(log_db.recording_id(), log_db);
    }

    fn handle_dropping_files(&mut self, egui_ctx: &egui::Context) {
        preview_files_being_dropped(egui_ctx);

        // Collect dropped files:
        if egui_ctx.input().raw.dropped_files.len() > 2 {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description("Can only load one file at a time")
                .show();
        }
        if let Some(file) = egui_ctx.input().raw.dropped_files.first() {
            if let Some(bytes) = &file.bytes {
                let mut bytes: &[u8] = &(*bytes)[..];
                if let Some(log_db) = load_file_contents(&file.name, &mut bytes) {
                    self.show_log_db(log_db);

                    #[allow(clippy::needless_return)] // false positive on wasm32
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &file.path {
                if let Some(log_db) = load_file_path(path) {
                    self.show_log_db(log_db);
                }
            }
        }
    }
}

fn preview_files_being_dropped(egui_ctx: &egui::Context) {
    use egui::*;

    // Preview hovering files:
    if !egui_ctx.input().raw.hovered_files.is_empty() {
        use std::fmt::Write as _;

        let mut text = "Drop to load:\n".to_owned();
        for file in &egui_ctx.input().raw.hovered_files {
            if let Some(path) = &file.path {
                write!(text, "\n{}", path.display()).ok();
            } else if !file.mime.is_empty() {
                write!(text, "\n{}", file.mime).ok();
            }
        }

        let painter =
            egui_ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = egui_ctx.input().screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Heading.resolve(&egui_ctx.style()),
            Color32::WHITE,
        );
    }
}

// ------------------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum PanelSelection {
    #[default]
    Viewport,

    EventLog,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct AppState {
    /// Global options for the whole viewer.
    options: Options,

    /// Things that need caching.
    #[serde(skip)]
    cache: Caches,

    selected_rec_id: RecordingId,

    /// Configuration for the current recording (found in [`LogDb`]).
    recording_configs: IntMap<RecordingId, RecordingConfig>,

    blueprints: HashMap<ApplicationId, crate::ui::Blueprint>,

    panel_selection: PanelSelection,
    event_log_view: crate::event_log_view::EventLogView,
    selection_panel: crate::selection_panel::SelectionPanel,
    time_panel: crate::time_panel::TimePanel,

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    #[serde(skip)]
    profiler: crate::Profiler,

    // TODO(emilk): use an image cache
    #[serde(skip)]
    static_image_cache: StaticImageCache,
}

impl AppState {
    fn show(
        &mut self,
        egui_ctx: &egui::Context,
        log_db: &LogDb,
        design_tokens: &DesignTokens,
        render_ctx: &mut re_renderer::RenderContext,
    ) {
        crate::profile_function!();

        let Self {
            options,
            cache,
            selected_rec_id,
            recording_configs,
            panel_selection,
            event_log_view,
            blueprints,
            selection_panel,
            time_panel,
            #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
                profiler: _,
            static_image_cache: _,
        } = self;

        let rec_cfg = recording_configs.entry(*selected_rec_id).or_default();
        let selected_app_id = log_db
            .recording_info()
            .map_or_else(ApplicationId::unknown, |rec_info| {
                rec_info.application_id.clone()
            });

        let mut ctx = ViewerContext {
            options,
            cache,
            log_db,
            rec_cfg,
            design_tokens,
            render_ctx,
        };

        let blueprint = blueprints.entry(selected_app_id.clone()).or_default();
        selection_panel.show_panel(&mut ctx, blueprint, egui_ctx);
        time_panel.show_panel(&mut ctx, blueprint, egui_ctx);

        let central_panel_frame = egui::Frame {
            fill: egui_ctx.style().visuals.window_fill(),
            inner_margin: egui::style::Margin::same(0.0),
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(central_panel_frame)
            .show(egui_ctx, |ui| match *panel_selection {
                PanelSelection::Viewport => blueprints
                    .entry(selected_app_id)
                    .or_default()
                    .blueprint_panel_and_viewport(&mut ctx, ui),
                PanelSelection::EventLog => event_log_view.ui(&mut ctx, ui),
            });

        // move time last, so we get to see the first data first!
        ctx.rec_cfg
            .time_ctrl
            .move_time(egui_ctx, &log_db.time_points);

        if WATERMARK {
            self.watermark(egui_ctx);
        }
    }

    fn watermark(&mut self, egui_ctx: &egui::Context) {
        use egui::*;
        let logo = self
            .static_image_cache
            .rerun_logo(&egui_ctx.style().visuals);
        let screen_rect = egui_ctx.input().screen_rect;
        let size = logo.size_vec2();
        let rect = Align2::RIGHT_BOTTOM
            .align_size_within_rect(size, screen_rect)
            .translate(-Vec2::splat(16.0));
        let mut mesh = Mesh::with_texture(logo.texture_id(egui_ctx));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
        egui_ctx.debug_painter().add(Shape::mesh(mesh));
    }
}

fn top_panel(egui_ctx: &egui::Context, frame: &mut eframe::Frame, app: &mut App) {
    crate::profile_function!();

    let panel_frame = {
        egui::Frame {
            inner_margin: egui::style::Margin::symmetric(8.0, 2.0),
            fill: app.design_tokens.top_bar_color,
            ..Default::default()
        }
    };

    let gui_zoom = if let Some(native_pixels_per_point) = frame.info().native_pixels_per_point {
        native_pixels_per_point / egui_ctx.pixels_per_point()
    } else {
        1.0
    };

    // On Mac, we share the same space as the native red/yellow/green close/minimize/maximize buttons.
    // This means we need to make room for them.
    let native_buttons_size_in_native_scale = egui::vec2(64.0, 24.0); // source: I measured /emilk

    let bar_height = if crate::FULLSIZE_CONTENT {
        // Use more vertical space when zoomed in…
        let bar_height = native_buttons_size_in_native_scale.y;

        // …but never shrink below the native button height when zoomed out.
        bar_height.max(gui_zoom * native_buttons_size_in_native_scale.y)
    } else {
        egui_ctx.style().spacing.interact_size.y
    };

    egui::TopBottomPanel::top("top_bar")
        .frame(panel_frame)
        .exact_height(bar_height)
        .show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.set_height(bar_height);

                if crate::FULLSIZE_CONTENT {
                    // Always use the same width measured in native GUI coordinates:
                    ui.add_space(gui_zoom * native_buttons_size_in_native_scale.x);
                }

                #[cfg(not(target_arch = "wasm32"))]
                ui.menu_button("File", |ui| {
                    file_menu(ui, app, frame);
                });

                ui.menu_button("View", |ui| {
                    view_menu(ui, app, frame);
                });

                ui.menu_button("Recordings", |ui| {
                    recordings_menu(ui, app);
                });

                #[cfg(debug_assertions)]
                ui.menu_button("Debug", |ui| {
                    debug_menu(ui);
                });

                ui.separator();

                if !app.log_db().is_empty() {
                    ui.selectable_value(
                        &mut app.state.panel_selection,
                        PanelSelection::Viewport,
                        "Viewport",
                    );

                    ui.selectable_value(
                        &mut app.state.panel_selection,
                        PanelSelection::EventLog,
                        "Event Log",
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !WATERMARK {
                        let logo = app.state.static_image_cache.rerun_logo(ui.visuals());
                        let response = ui
                            .add(egui::ImageButton::new(
                                logo.texture_id(egui_ctx),
                                logo.size_vec2() * 16.0 / logo.size_vec2().y,
                            ))
                            .on_hover_text("https://rerun.io");
                        if response.clicked() {
                            ui.output().open_url =
                                Some(egui::output::OpenUrl::new_tab("https://rerun.io"));
                        }
                    }
                    egui::warn_if_debug_build(ui);
                });
            });
        });
}

// ---

const FILE_SAVER_PROMISE: &str = "file_saver";
const FILE_SAVER_NOTIF_DURATION: Option<std::time::Duration> =
    Some(std::time::Duration::from_secs(4));

fn file_saver_progress_ui(egui_ctx: &egui::Context, app: &mut App) {
    use std::path::PathBuf;

    let file_save_in_progress = app.promise_exists(FILE_SAVER_PROMISE);
    if file_save_in_progress {
        // There's already a file save running in the background.

        if let Some(res) = app.poll_promise::<anyhow::Result<PathBuf>>(FILE_SAVER_PROMISE) {
            // File save promise has returned.

            match res {
                Ok(path) => {
                    let msg = format!("File saved to {path:?}.");
                    re_log::info!(msg);
                    app.toasts.info(msg).set_duration(FILE_SAVER_NOTIF_DURATION);
                }
                Err(err) => {
                    let msg = format!("{err}");
                    re_log::error!(msg);
                    app.toasts
                        .error(msg)
                        .set_duration(FILE_SAVER_NOTIF_DURATION);
                }
            }
        } else {
            // File save promise is still running in the background.

            // NOTE: not a toast, want something a bit more discreet here.
            egui::Window::new("file_saver_spin")
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::ZERO)
                .title_bar(false)
                .enabled(false)
                .auto_sized()
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Writing file to disk…");
                    })
                });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn file_menu(ui: &mut egui::Ui, app: &mut App, frame: &mut eframe::Frame) {
    // TODO(emilk): support saving data on web
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file_save_in_progress = app.promise_exists(FILE_SAVER_PROMISE);
        if file_save_in_progress {
            ui.add_enabled_ui(false, |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.button("Save…");
                    ui.spinner();
                });
                ui.horizontal(|ui| {
                    let _ = ui.button("Save Time Selection…");
                    ui.spinner();
                });
            });
        } else {
            let (clicked, time_selection) = ui
                .add_enabled_ui(!app.log_db().is_empty(), |ui| {
                    if ui
                        .button("Save…")
                        .on_hover_text("Save all data to a Rerun data file (.rrd)")
                        .clicked()
                    {
                        return (true, None);
                    }

                    // We need to know the time selection _before_ we can even display the
                    // button, as this will determine wether its grayed out or not!
                    // TODO(cmc): In practice the loop (green) selection is always there
                    // at the moment so...
                    let time_selection = app
                        .state
                        .recording_configs
                        .get(&app.state.selected_rec_id)
                        // is there an active time selection?
                        .and_then(|rec_cfg| {
                            rec_cfg
                                .time_ctrl
                                .time_selection()
                                .map(|q| (*rec_cfg.time_ctrl.timeline(), q))
                        });

                    if ui
                        .add_enabled(
                            time_selection.is_some(),
                            egui::Button::new("Save time selection…"),
                        )
                        .on_hover_text(
                            "Save data for the current time selection to a Rerun data file (.rrd)",
                        )
                        .clicked()
                    {
                        return (true, time_selection);
                    }

                    (false, None)
                })
                .inner;

            if clicked {
                // User clicked the Save button, there is no other file save running, and
                // the DB isn't empty: let's spawn a new one.

                if let Some(path) = rfd::FileDialog::new().set_file_name("data.rrd").save_file() {
                    let f = save_database_to_file(app, path, time_selection);
                    if let Err(err) = app.spawn_threaded_promise(FILE_SAVER_PROMISE, f) {
                        // NOTE: Shouldn't even be possible as the "Save" button is already
                        // grayed out at this point... better safe than sorry though.
                        app.toasts
                            .error(err.to_string())
                            .set_duration(FILE_SAVER_NOTIF_DURATION);
                    }
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    if ui
        .button("Load…")
        .on_hover_text("Load a Rerun Data File (.rrd)")
        .clicked()
    {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("rerun data file", &["rrd"])
            .pick_file()
        {
            if let Some(log_db) = load_file_path(&path) {
                app.show_log_db(log_db);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Quit").clicked() {
        frame.close();
    }
}

fn view_menu(ui: &mut egui::Ui, app: &mut App, frame: &mut eframe::Frame) {
    ui.set_min_width(180.0);

    // On the web the browser controls the zoom
    if !frame.is_web() {
        egui::gui_zoom::zoom_menu_buttons(ui, frame.info().native_pixels_per_point);
        ui.separator();
    }

    if ui
        .add(
            egui::Button::new("Reset Viewer")
                .shortcut_text(ui.ctx().format_shortcut(&kb_shortcuts::RESET_VIEWER)),
        )
        .on_hover_text("Reset the viewer to how it looked the first time you ran it")
        .clicked()
    {
        app.reset(ui.ctx());
        ui.close_menu();
    }

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    if ui
        .add(
            egui::Button::new("Profile Viewer")
                .shortcut_text(ui.ctx().format_shortcut(&kb_shortcuts::SHOW_PROFILER)),
        )
        .on_hover_text("Starts a profiler, showing what makes the viewer run slow")
        .clicked()
    {
        app.state.profiler.start();
    }

    if ui
        .add(
            egui::Button::new("Toggle Memory Panel")
                .shortcut_text(ui.ctx().format_shortcut(&kb_shortcuts::TOGGLE_MEMORY_PANEL)),
        )
        .clicked()
    {
        app.memory_panel_open ^= true;
    }
}

// ---

fn recordings_menu(ui: &mut egui::Ui, app: &mut App) {
    let log_dbs = app
        .log_dbs
        .values()
        .sorted_by_key(|log_db| log_db.recording_info().map(|ri| ri.started))
        .collect_vec();

    if log_dbs.is_empty() {
        ui.weak("(empty)");
        return;
    }

    ui.style_mut().wrap = Some(false);
    for log_db in log_dbs {
        let info = if let Some(rec_info) = log_db.recording_info() {
            format!(
                "{} - {}",
                rec_info.recording_source,
                rec_info.started.format()
            )
        } else {
            "<UNKNOWN>".to_owned()
        };
        if ui
            .selectable_label(app.state.selected_rec_id == log_db.recording_id(), info)
            .clicked()
        {
            app.state.selected_rec_id = log_db.recording_id();
        }
    }
}

#[cfg(debug_assertions)]
fn debug_menu(ui: &mut egui::Ui) {
    ui.style_mut().wrap = Some(false);

    #[allow(clippy::manual_assert)]
    if ui.button("panic!").clicked() {
        panic!("Intentional panic");
    }

    if ui.button("panic! during unwind").clicked() {
        struct PanicOnDrop {}
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                panic!("Second intentional panic in Drop::drop");
            }
        }

        let _this_will_panic_when_dropped = PanicOnDrop {};
        panic!("First intentional panic");
    }
}

// ---

/// Returns a closure that, when run, will save the contents of the current database
/// to disk, at the specified `path`.
///
/// If `time_selection` is specified, then only data for that specific timeline over that
/// specific time range will be accounted for.
#[cfg(not(target_arch = "wasm32"))]
fn save_database_to_file(
    app: &mut App,
    path: std::path::PathBuf,
    time_selection: Option<(Timeline, TimeRangeF)>,
) -> impl FnOnce() -> anyhow::Result<std::path::PathBuf> {
    let msgs = match time_selection {
        // Fast path: no query, just dump everything.
        None => app
            .log_db()
            .chronological_log_messages()
            .cloned()
            .collect::<Vec<_>>(),
        // Query path: time to filter!
        Some((timeline, range)) => {
            use std::ops::RangeInclusive;
            let range: RangeInclusive<TimeInt> = range.min.floor()..=range.max.ceil();
            app.log_db()
                .chronological_log_messages()
                .filter(|msg| {
                    match msg {
                        LogMsg::BeginRecordingMsg(_) | LogMsg::TypeMsg(_) => true, // timeless
                        LogMsg::DataMsg(DataMsg { time_point, .. })
                        | LogMsg::PathOpMsg(PathOpMsg { time_point, .. }) => {
                            time_point.is_timeless() || {
                                let is_within_range = time_point
                                    .0
                                    .get(&timeline)
                                    .map_or(false, |t| range.contains(t));
                                is_within_range
                            }
                        }
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        }
    };

    move || {
        crate::profile_scope!("save_to_file");

        use anyhow::Context as _;
        let file = std::fs::File::create(path.as_path())
            .with_context(|| format!("Failed to create file at {:?}", path))?;

        re_log_types::encoding::encode(msgs.iter(), file).map(|_| path)
    }
}

#[allow(unused_mut)]
fn load_rrd_to_log_db(mut read: impl std::io::Read) -> anyhow::Result<LogDb> {
    crate::profile_function!();

    let decoder = re_log_types::encoding::Decoder::new(read)?;

    let mut log_db = LogDb::default();
    for msg in decoder {
        log_db.add(msg?);
    }
    Ok(log_db)
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
fn load_file_path(path: &std::path::Path) -> Option<LogDb> {
    fn load_file_path_impl(path: &std::path::Path) -> anyhow::Result<LogDb> {
        crate::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::open(path).context("Failed to open file")?;
        load_rrd_to_log_db(file)
    }

    re_log::info!("Loading {path:?}…");

    match load_file_path_impl(path) {
        Ok(new_log_db) => {
            re_log::info!("Loaded {path:?}");
            Some(new_log_db)
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
            None
        }
    }
}

#[must_use]
fn load_file_contents(name: &str, read: impl std::io::Read) -> Option<LogDb> {
    match load_rrd_to_log_db(read) {
        Ok(log_db) => {
            re_log::info!("Loaded {name:?}");
            Some(log_db)
        }
        Err(err) => {
            let msg = format!("Failed loading {name:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
            None
        }
    }
}

#[derive(Default)]
struct StaticImageCache {
    images: std::collections::HashMap<&'static str, RetainedImage>,
}

impl StaticImageCache {
    pub fn get(&mut self, name: &'static str, image_bytes: &[u8]) -> &RetainedImage {
        self.images.entry(name).or_insert_with(|| {
            RetainedImage::from_color_image(name, load_image_bytes(image_bytes).unwrap())
        })
    }

    pub fn rerun_logo(&mut self, visuals: &egui::Visuals) -> &RetainedImage {
        if visuals.dark_mode {
            self.get(
                "logo_dark_mode",
                include_bytes!("../data/logo_dark_mode.png"),
            )
        } else {
            self.get(
                "logo_light_mode",
                include_bytes!("../data/logo_light_mode.png"),
            )
        }
    }
}

pub fn load_image_bytes(image_bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let image = image::load_from_memory(image_bytes).map_err(|err| err.to_string())?;
    let image = image.into_rgba8();
    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}
