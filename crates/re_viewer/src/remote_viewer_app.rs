use crate::App;

/// Connects to a server over WebSockets.
pub struct RemoteViewerApp {
    build_info: re_build_info::BuildInfo,
    app_env: crate::AppEnvironment,
    startup_options: crate::StartupOptions,
    re_ui: re_ui::ReUi,

    /// The url of the remote server.
    url: String,

    app: Option<(re_ws_comms::Connection, App)>,
}

impl RemoteViewerApp {
    /// url to rerun server
    pub fn new(
        build_info: re_build_info::BuildInfo,
        app_env: crate::AppEnvironment,
        startup_options: crate::StartupOptions,
        re_ui: re_ui::ReUi,
        storage: Option<&dyn eframe::Storage>,
        url: String,
    ) -> Self {
        let mut slf = Self {
            build_info,
            app_env,
            startup_options,
            re_ui,
            url,
            app: None,
        };
        slf.connect(storage);
        slf
    }

    fn connect(&mut self, storage: Option<&dyn eframe::Storage>) {
        let (tx, rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::WsClient {
                ws_server_url: self.url.clone(),
            },
            re_smart_channel::SmartChannelSource::WsClient {
                ws_server_url: self.url.clone(),
            },
        );

        let egui_ctx = self.re_ui.egui_ctx.clone();

        re_log::info!("Connecting to WS server at {:?}…", self.url);

        let callback = move |binary: Vec<u8>| {
            match re_ws_comms::decode_log_msg(&binary) {
                Ok(log_msg) => {
                    if tx.send(log_msg).is_ok() {
                        // Spend a few more milliseconds decoding incoming messages,
                        // then trigger a repaint (#963):
                        egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));

                        std::ops::ControlFlow::Continue(())
                    } else {
                        re_log::info!("Failed to send log message to viewer - closing");
                        std::ops::ControlFlow::Break(())
                    }
                }
                Err(err) => {
                    re_log::error!("Failed to parse message: {err}");
                    std::ops::ControlFlow::Break(())
                }
            }
        };

        match re_ws_comms::Connection::viewer_to_server(self.url.clone(), callback) {
            Ok(connection) => {
                let app = crate::App::from_receiver(
                    self.build_info,
                    &self.app_env,
                    self.startup_options,
                    self.re_ui.clone(),
                    storage,
                    rx,
                );

                self.app = Some((connection, app));
            }
            Err(err) => {
                re_log::error!("Failed to connect to {:?}: {err}", self.url);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_profiler(&mut self, profiler: crate::Profiler) {
        if let Some((_, app)) = &mut self.app {
            app.set_profiler(profiler);
        }
    }
}

impl eframe::App for RemoteViewerApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent so we can get rounded corners when doing [`re_ui::CUSTOM_WINDOW_DECORATIONS`]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some((_, app)) = &mut self.app {
            app.save(storage);
        }
    }

    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        if false {
            // TODO(emilk): move this url selection into the main app.
            // but for now, just remove it, because it is ugly (see https://github.com/rerun-io/rerun/issues/1079).
            egui::TopBottomPanel::top("server").show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    if ui.text_edit_singleline(&mut self.url).lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        if let Some(storage) = frame.storage_mut() {
                            if let Some((_, mut app)) = self.app.take() {
                                app.save(storage);
                            }
                        }
                        self.connect(frame.storage());
                    }
                });
            });
        }

        if let Some((_, app)) = &mut self.app {
            app.update(egui_ctx, frame);
        } else {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // TODO(emilk): show the error message.
                ui.label("An error occurred.\nCheck the debug console for details.");
            });
        }
    }
}
