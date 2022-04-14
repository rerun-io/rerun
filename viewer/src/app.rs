use eframe::egui;

use log_types::*;

use crate::LogDb;

// ----------------------------------------------------------------------------

pub struct App {
    rx: std::sync::mpsc::Receiver<LogMsg>,
    /// Where the logs are stored.
    log_db: LogDb,

    state: AppState,
}

impl App {
    pub fn new(
        storage: Option<&dyn eframe::Storage>,
        rx: std::sync::mpsc::Receiver<LogMsg>,
    ) -> Self {
        let state = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        Self {
            rx,
            log_db: Default::default(),
            state,
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(log_msg) = self.rx.try_recv() {
            self.log_db.add(log_msg);
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
}

impl AppState {
    fn show(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame, log_db: &mut LogDb) {
        crate::profile_function!();

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
        } = self;

        egui::TopBottomPanel::bottom("time_panel")
            .resizable(true)
            .default_height(210.0)
            .show(egui_ctx, |ui| {
                time_panel.ui(log_db, context, ui);
            });

        egui::SidePanel::right("context").show(egui_ctx, |ui| {
            context_panel.ui(log_db, context, ui);
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
        log_types::encoding::encode(log_db.messages(), file)
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
fn load_file_path(path: &std::path::PathBuf, log_db: &mut LogDb) {
    fn load_file_path_impl(path: &std::path::PathBuf) -> anyhow::Result<LogDb> {
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
