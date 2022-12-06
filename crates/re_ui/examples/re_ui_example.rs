const FULLSIZE_CONTENT: bool = true;

fn main() {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some([1200.0, 800.0].into()),
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        #[cfg(target_os = "macos")]
        fullsize_content: FULLSIZE_CONTENT,

        ..Default::default()
    };

    eframe::run_native(
        "re_ui example app",
        native_options,
        Box::new(move |cc| {
            let _re_ui = re_ui::ReUi::load_and_apply(&cc.egui_ctx);
            Box::new(TemplateApp { _re_ui })
        }),
    );
}

pub struct TemplateApp {
    _re_ui: re_ui::ReUi,
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.strong("re_ui example app");
            })
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::warn_if_debug_build(ui);
            ui.label("Hello world!");
        });
    }
}
