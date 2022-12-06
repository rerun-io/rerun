fn main() {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1200.0, 800.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: re_ui::FULLSIZE_CONTENT,

        ..Default::default()
    };

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            let re_ui = re_ui::ReUi::load_and_apply(&cc.egui_ctx);
            Box::new(ExampleApp { re_ui })
        }),
    );
}

pub struct ExampleApp {
    re_ui: re_ui::ReUi,
}

impl eframe::App for ExampleApp {
    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.top_bar(egui_ctx, frame);

        egui::CentralPanel::default().show(egui_ctx, |ui| {
            egui::warn_if_debug_build(ui);
            ui.label("Hello world!");
        });
    }
}

impl ExampleApp {
    fn top_bar(&self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let panel_frame = {
            egui::Frame {
                inner_margin: egui::style::Margin::symmetric(8.0, 2.0),
                fill: self.re_ui.design_tokens.top_bar_color,
                ..Default::default()
            }
        };

        let native_pixels_per_point = frame.info().native_pixels_per_point;
        let fullscreen = {
            #[cfg(target_os = "macos")]
            {
                frame.info().window_info.fullscreen
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        };
        let top_bar_style = self
            .re_ui
            .top_bar_style(native_pixels_per_point, fullscreen);

        egui::TopBottomPanel::top("top_bar")
            .frame(panel_frame)
            .exact_height(top_bar_style.height)
            .show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.set_height(top_bar_style.height);
                    ui.add_space(top_bar_style.indent);

                    ui.centered_and_justified(|ui| {
                        ui.strong("re_ui example app");
                    })
                });
            });
    }
}
