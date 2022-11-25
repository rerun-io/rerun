use crate::{design_tokens::DesignTokens, App};

/// Connects to a server over `WebSockets`.
pub struct RemoteViewerApp {
    design_tokens: DesignTokens,
    url: String,
    app: Option<(re_ws_comms::Connection, App)>,
}

impl RemoteViewerApp {
    /// url to rerun server
    pub fn new(
        egui_ctx: &egui::Context,
        design_tokens: crate::design_tokens::DesignTokens,
        storage: Option<&dyn eframe::Storage>,
        url: String,
    ) -> Self {
        let mut slf = Self {
            design_tokens,
            url,
            app: None,
        };
        slf.connect(egui_ctx, storage);
        slf
    }

    fn connect(&mut self, egui_ctx: &egui::Context, storage: Option<&dyn eframe::Storage>) {
        let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Network);

        let egui_ctx_clone = egui_ctx.clone();

        re_log::info!("Connecting to WS server at {:?}…", self.url);

        let connection =
            re_ws_comms::Connection::viewer_to_server(self.url.clone(), move |binary: Vec<u8>| {
                match re_ws_comms::decode_log_msg(&binary) {
                    Ok(log_msg) => {
                        if tx.send(log_msg).is_ok() {
                            egui_ctx_clone.request_repaint(); // Wake up UI thread
                            std::ops::ControlFlow::Continue(())
                        } else {
                            re_log::info!("Failed to send log message to viewer - closing");
                            std::ops::ControlFlow::Break(())
                        }
                    }
                    Err(err) => {
                        re_log::error!("Failed to parse message: {}", re_error::format(&err));
                        std::ops::ControlFlow::Break(())
                    }
                }
            })
            .unwrap(); // TODO(emilk): handle error

        let app = crate::App::from_receiver(egui_ctx, self.design_tokens, storage, rx);

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
