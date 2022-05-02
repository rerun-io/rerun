use egui_extras::RetainedImage;
use log_types::*;

use crate::LogDb;

const WATERMARK: bool = false; // Nice for recording media material

// ----------------------------------------------------------------------------

pub struct App {
    rx: Option<std::sync::mpsc::Receiver<LogMsg>>,

    /// Where the logs are stored.
    log_db: LogDb,

    state: AppState,
}

impl App {
    /// Create a viewer that receives new log messages over time
    pub fn from_receiver(
        storage: Option<&dyn eframe::Storage>,
        rx: std::sync::mpsc::Receiver<LogMsg>,
    ) -> Self {
        let state = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        Self {
            rx: Some(rx),
            log_db: Default::default(),
            state,
        }
    }

    pub(crate) fn from_log_db(storage: Option<&dyn eframe::Storage>, log_db: LogDb) -> Self {
        let state = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        Self {
            rx: None,
            log_db,
            state,
        }
    }

    /// load a `.rrd` data file.
    pub fn from_rrd_path(storage: Option<&dyn eframe::Storage>, path: &std::path::Path) -> Self {
        let mut log_db = Default::default();
        load_file_path(path, &mut log_db);
        Self::from_log_db(storage, log_db)
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(rx) = &mut self.rx {
            while let Ok(log_msg) = rx.try_recv() {
                self.log_db.add(log_msg);
            }
        }

        self.state.show(egui_ctx, frame, &mut self.log_db);

        self.handle_dropping_files(egui_ctx);
    }
}

impl App {
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
                load_file_contents(&file.name, &mut bytes, &mut self.log_db);
                return;
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &file.path {
                load_file_path(path, &mut self.log_db);
            }
        }
    }
}

fn preview_files_being_dropped(ctx: &egui::Context) {
    use egui::*;

    // Preview hovering files:
    if !ctx.input().raw.hovered_files.is_empty() {
        let mut text = "Drop to load:\n".to_owned();
        for file in &ctx.input().raw.hovered_files {
            if let Some(path) = &file.path {
                text += &format!("\n{}", path.display());
            } else if !file.mime.is_empty() {
                text += &format!("\n{}", file.mime);
            }
        }

        let painter =
            ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = ctx.input().screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Heading.resolve(&ctx.style()),
            Color32::WHITE,
        );
    }
}

// ------------------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct AppState {
    context: crate::ViewerContext,
    view_index: usize,
    log_table_view: crate::log_table_view::LogTableView,
    space_view: crate::space_view::SpaceView,
    context_panel: crate::context_panel::ContextPanel,
    time_panel: crate::time_panel::TimePanel,

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    #[serde(skip)]
    profiler: crate::misc::profiler::Profiler,

    // TODO: use an image cache
    #[serde(skip)]
    static_image_cache: StaticImageCache,
}

impl AppState {
    fn show(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame, log_db: &mut LogDb) {
        crate::profile_function!();

        self.context.on_frame_start(log_db);

        egui::TopBottomPanel::top("View").show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    self.file_menu(ui, frame, log_db);
                });

                ui.separator();

                egui::widgets::global_dark_light_mode_switch(ui);

                ui.separator();

                ui.selectable_value(&mut self.view_index, 0, "Spaces");
                ui.selectable_value(&mut self.view_index, 1, "Table");

                ui.with_layout(egui::Layout::right_to_left(), |ui| {
                    if !WATERMARK {
                        let logo = self.static_image_cache.rerun_logo(ui.visuals());
                        logo.show_max_size(ui, [500.0, 16.0].into());
                    }
                    egui::warn_if_debug_build(ui);
                });
            });
        });

        let Self {
            context,
            view_index,
            log_table_view,
            space_view,
            context_panel,
            time_panel,
            #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
                profiler: _,
            static_image_cache: _,
        } = self;

        egui::SidePanel::right("context").show(egui_ctx, |ui| {
            context_panel.ui(log_db, context, ui);
        });

        egui::TopBottomPanel::bottom("time_panel")
            .resizable(true)
            .default_height(210.0)
            .show(egui_ctx, |ui| {
                time_panel.ui(log_db, context, ui);
            });

        egui::CentralPanel::default().show(egui_ctx, |ui| match view_index {
            0 => space_view.ui(log_db, context, ui),
            1 => log_table_view.ui(log_db, context, ui),
            _ => {}
        });

        // move time last, so we get to see the first data first!
        context
            .time_control
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

    fn file_menu(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame, _log_db: &mut LogDb) {
        // TODO: support saving data on web
        #[cfg(not(target_arch = "wasm32"))]
        if ui.button("Save…").on_hover_text("Save all data").clicked() {
            if let Some(path) = rfd::FileDialog::new().set_file_name("data.rrd").save_file() {
                save_to_file(_log_db, &path);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        if ui.button("Load").on_hover_text("Save all data").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("rerun data file", &["rrd"])
                .pick_file()
            {
                load_file_path(&path, _log_db);
            }
        }

        ui.menu_button("Advanced", |ui| {
            if ui
                .button("Reset viewer")
                .on_hover_text("Reset the viewer to how it looked the first time you ran it.")
                .clicked()
            {
                *self = Default::default();

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
                self.profiler.start();
            }
        });

        if ui.button("Quit").clicked() {
            frame.quit();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_to_file(log_db: &LogDb, path: &std::path::PathBuf) {
    fn save_to_file_impl(log_db: &LogDb, path: &std::path::PathBuf) -> anyhow::Result<()> {
        crate::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::create(path).context("Failed to create file")?;
        log_types::encoding::encode(log_db.chronological_messages(), file)
    }

    match save_to_file_impl(log_db, path) {
        // TODO: show a popup instead of logging result
        Ok(()) => {
            tracing::info!("Data saved to {:?}", path);
        }
        Err(err) => {
            let msg = format!("Failed saving data to {path:?}: {err}");
            tracing::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
        }
    }
}

#[allow(unused_mut)]
fn load_rrd(mut read: impl std::io::Read) -> anyhow::Result<LogDb> {
    crate::profile_function!();

    #[cfg(target_arch = "wasm32")]
    let decoder = log_types::encoding::Decoder::new(&mut read)?;

    #[cfg(not(target_arch = "wasm32"))]
    let decoder = log_types::encoding::Decoder::new(read)?;

    let mut log_db = LogDb::default();
    for msg in decoder {
        log_db.add(msg?);
    }
    Ok(log_db)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_file_path(path: &std::path::Path, log_db: &mut LogDb) {
    fn load_file_path_impl(path: &std::path::Path) -> anyhow::Result<LogDb> {
        crate::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::open(path).context("Failed to open file")?;
        load_rrd(file)
    }

    match load_file_path_impl(path) {
        Ok(new_log_db) => {
            tracing::info!("Loaded {path:?}");
            *log_db = new_log_db;
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {err}");
            tracing::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
        }
    }
}

fn load_file_contents(name: &str, read: impl std::io::Read, log_db: &mut LogDb) {
    match load_rrd(read) {
        Ok(new_log_db) => {
            tracing::info!("Loaded {name:?}");
            *log_db = new_log_db;
        }
        Err(err) => {
            let msg = format!("Failed loading {name:?}: {err}");
            tracing::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
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
    let image = image.to_rgba8();
    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}
