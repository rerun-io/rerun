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
        self.state.show(egui_ctx, frame, &self.log_db);
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
}

impl AppState {
    fn show(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame, log_db: &LogDb) {
        egui::TopBottomPanel::top("View").show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.menu_button("Advanced", |ui| {
                        if ui
                            .button("Reset viewer state")
                            .on_hover_text(
                                "Reset the viewer to how it looked the first time you ran it",
                            )
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
                    });

                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
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
}
