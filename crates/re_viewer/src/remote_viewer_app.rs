use crate::App;

/// Connects to a server over `WebSockets`.
#[derive(Default)]
pub struct RemoteViewerApp {
    url: String,
    app: Option<(re_ws_comms::Connection, App)>,
}

impl RemoteViewerApp {
    /// url to rerun server
    pub fn new(
        egui_ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        url: String,
    ) -> Self {
        let mut slf = Self { url, app: None };
        slf.connect(egui_ctx, storage);
        slf
    }

    fn connect(&mut self, egui_ctx: &egui::Context, storage: Option<&dyn eframe::Storage>) {
        let (tx, rx) = std::sync::mpsc::channel();

        let egui_ctx_clone = egui_ctx.clone();

        let connection = re_ws_comms::Connection::viewer_to_server(
            self.url.clone(),
            move |data_msg: re_log_types::LogMsg| {
                if tx.send(data_msg).is_ok() {
                    egui_ctx_clone.request_repaint(); // Wake up UI thread
                    std::ops::ControlFlow::Continue(())
                } else {
                    re_log::info!("Failed to send log message to viewer - closing");
                    std::ops::ControlFlow::Break(())
                }
            },
        )
        .unwrap(); // TODO(emilk): handle error

        let app = crate::App::from_receiver(egui_ctx, storage, rx);

        self.app = Some((connection, app));
    }

    #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        if let Some((_, app)) = &mut self.app {
            app.set_profiler(profiler);
        }
    }
}

impl eframe::App for RemoteViewerApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some((_, app)) = &mut self.app {
            app.save(storage);
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("server").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                if ui.text_edit_singleline(&mut self.url).lost_focus()
                    && ui.input().key_pressed(egui::Key::Enter)
                {
                    if let Some(storage) = frame.storage_mut() {
                        if let Some((_, mut app)) = self.app.take() {
                            app.save(storage);
                        }
                    }
                    self.connect(egui_ctx, frame.storage());
                }
            });
        });

        if let Some((_, app)) = &mut self.app {
            app.update(egui_ctx, frame);
        }
    }
}
