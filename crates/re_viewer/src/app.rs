use std::sync::mpsc::Receiver;

use crate::misc::{Caches, Options, RecordingConfig, ViewerContext};
use egui_extras::RetainedImage;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_data_store::log_db::LogDb;
use re_log_types::*;

const WATERMARK: bool = false; // Nice for recording media material

// ----------------------------------------------------------------------------

/// The Rerun viewer as an [`eframe`] application.
pub struct App {
    rx: Option<Receiver<LogMsg>>,

    /// Where the logs are stored.
    log_dbs: IntMap<RecordingId, LogDb>,

    /// What is serialized
    state: AppState,

    /// Set to `true` on Ctrl-C.
    #[cfg(not(target_arch = "wasm32"))]
    ctrl_c: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        egui_ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        rx: Receiver<LogMsg>,
    ) -> Self {
        Self::new(egui_ctx, storage, Some(rx), Default::default())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_log_db(
        egui_ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        log_db: LogDb,
    ) -> Self {
        Self::new(egui_ctx, storage, None, log_db)
    }

    /// load a `.rrd` data file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_rrd_path(
        egui_ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        path: &std::path::Path,
    ) -> Self {
        let log_db = load_file_path(path).unwrap_or_default(); // TODO(emilk): exit on error.
        Self::from_log_db(egui_ctx, storage, log_db)
    }

    fn new(
        egui_ctx: &egui::Context,
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
            let egui_ctx = egui_ctx.clone();

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
            rx,
            log_dbs,
            state,
            #[cfg(not(target_arch = "wasm32"))]
            ctrl_c,
        }
    }

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        self.state.profiler = profiler;
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.ctrl_c.load(std::sync::atomic::Ordering::SeqCst) {
            frame.close();
            return;
        }

        self.state.cache.new_frame();

        if let Some(rx) = &mut self.rx {
            crate::profile_scope!("receive_messages");
            let start = instant::Instant::now();
            while let Ok(msg) = rx.try_recv() {
                if let LogMsg::BeginRecordingMsg(msg) = &msg {
                    re_log::info!("Begginning a new recording: {:?}", msg.info);
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

        {
            // Cleanup:
            self.log_dbs.retain(|_, log_db| !log_db.is_empty());

            if !self.log_dbs.contains_key(&self.state.selected_rec_id) {
                self.state.selected_rec_id =
                    self.log_dbs.keys().next().cloned().unwrap_or_default();
            }

            // Make sure we don't persist old stuff we don't need:
            self.state
                .recording_configs
                .retain(|recording_id, _| self.log_dbs.contains_key(recording_id));
            self.state
                .viewport_panel
                .retain(|recording_id, _| self.log_dbs.contains_key(recording_id));
        }

        top_panel(egui_ctx, frame, self);

        let log_db = self.log_dbs.entry(self.state.selected_rec_id).or_default();

        self.state
            .recording_configs
            .entry(self.state.selected_rec_id)
            .or_default()
            .on_frame_start(log_db);

        if log_db.is_empty() && self.rx.is_some() {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.heading("Waiting for data…"); // TODO(emilk): show what ip/port we are listening to
                });
            });
        } else {
            self.state.show(egui_ctx, log_db);
        }

        self.handle_dropping_files(egui_ctx);
    }
}

impl App {
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
    viewport_panel: IntMap<RecordingId, crate::viewport_panel::ViewportPanel>,

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
    fn show(&mut self, egui_ctx: &egui::Context, log_db: &LogDb) {
        crate::profile_function!();

        let Self {
            options,
            cache,
            selected_rec_id: selected_recording_id,
            recording_configs,
            panel_selection,
            event_log_view,
            viewport_panel,
            selection_panel,
            time_panel,
            #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
                profiler: _,
            static_image_cache: _,
        } = self;

        let rec_cfg = recording_configs.entry(*selected_recording_id).or_default();

        let mut ctx = ViewerContext {
            options,
            cache,
            log_db,
            rec_cfg,
        };

        if ctx.rec_cfg.selection.is_some() {
            egui::SidePanel::right("selection_view").show(egui_ctx, |ui| {
                selection_panel.ui(&mut ctx, ui);
            });
        }

        egui::TopBottomPanel::bottom("time_panel")
            .resizable(true)
            .default_height(210.0)
            .show(egui_ctx, |ui| {
                time_panel.ui(&mut ctx, ui);
            });

        egui::CentralPanel::default().show(egui_ctx, |ui| match *panel_selection {
            PanelSelection::Viewport => viewport_panel
                .entry(*selected_recording_id)
                .or_default()
                .ui(&mut ctx, ui),
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

    egui::TopBottomPanel::top("top_bar").show(egui_ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                file_menu(ui, app, frame);
            });

            ui.menu_button("Recordings", |ui| {
                recordings_menu(ui, app);
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

fn file_menu(ui: &mut egui::Ui, app: &mut App, _frame: &mut eframe::Frame) {
    // TODO(emilk): support saving data on web
    #[cfg(not(target_arch = "wasm32"))]
    {
        let log_db = app.log_db();
        if ui
            .add_enabled(!log_db.is_empty(), egui::Button::new("Save…"))
            .on_hover_text("Save all data to a Rerun data file (.rrd)")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new().set_file_name("data.rrd").save_file() {
                save_to_file(log_db, &path);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    if ui
        .button("Load")
        .on_hover_text("Load a Rerun data file (.rrd)")
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

    ui.menu_button("Advanced", |ui| {
        if ui
            .button("Reset viewer")
            .on_hover_text("Reset the viewer to how it looked the first time you ran it.")
            .clicked()
        {
            app.state = Default::default();

            // Keep dark/light mode setting:
            let is_dark_mode = ui.ctx().style().visuals.dark_mode;
            *ui.ctx().memory() = Default::default();
            ui.ctx().set_visuals(if is_dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            ui.close_menu();
        }

        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        if ui
            .button("Profile viewer")
            .on_hover_text("Starts a profiler, showing what makes the viewer run slow")
            .clicked()
        {
            app.state.profiler.start();
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Quit").clicked() {
        _frame.close();
    }
}

fn recordings_menu(ui: &mut egui::Ui, app: &mut App) {
    let log_dbs = app
        .log_dbs
        .values()
        .sorted_by_key(|log_db| log_db.recording_info().map(|ri| ri.started))
        .collect_vec();

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

#[cfg(not(target_arch = "wasm32"))]
fn save_to_file(log_db: &LogDb, path: &std::path::PathBuf) {
    fn save_to_file_impl(log_db: &LogDb, path: &std::path::PathBuf) -> anyhow::Result<()> {
        crate::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::create(path).context("Failed to create file")?;
        re_log_types::encoding::encode(log_db.chronological_log_messages(), file)
    }

    match save_to_file_impl(log_db, path) {
        // TODO(emilk): show a popup instead of logging result
        Ok(()) => {
            re_log::info!("Data saved to {:?}", path);
        }
        Err(err) => {
            let msg = format!("Failed saving data to {path:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
        }
    }
}

#[allow(unused_mut)]
fn load_rrd_to_log_db(mut read: impl std::io::Read) -> anyhow::Result<LogDb> {
    crate::profile_function!();

    #[cfg(target_arch = "wasm32")]
    let decoder = re_log_types::encoding::Decoder::new(&mut read)?;

    #[cfg(not(target_arch = "wasm32"))]
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
