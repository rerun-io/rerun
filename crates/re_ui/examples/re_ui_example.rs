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
            // let design_tokens = crate::customize_eframe(cc);
            Box::new(TemplateApp::default())
        }),
    );
}

#[derive(Default)]
pub struct TemplateApp {}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
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
